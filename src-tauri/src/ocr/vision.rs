//! macOS backend: Apple Vision `VNRecognizeTextRequest` (macOS 10.15+), run
//! entirely on-device.

use image::RgbaImage;
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, sel};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSLocale, NSObjectProtocol, NSString};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
};

use super::TextBox;
use crate::imaging::encode::encode_preview;

pub fn recognize(image: &RgbaImage) -> Result<Vec<TextBox>, String> {
    // Vision decodes the image itself; an in-memory PNG is the simplest
    // lossless hand-off. It is never written to disk.
    let png = encode_preview(image)?;
    let (w, h) = (f64::from(image.width()), f64::from(image.height()));

    autoreleasepool(|_| {
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        if let Some(languages) = recognition_languages(&request) {
            request.setRecognitionLanguages(&languages);
        }
        // macOS 13+: let Vision pick the script per image.
        if request.respondsToSelector(sel!(setAutomaticallyDetectsLanguage:)) {
            request.setAutomaticallyDetectsLanguage(true);
        }

        let data = NSData::with_bytes(&png);
        let options = NSDictionary::<NSString, AnyObject>::new();
        let handler =
            VNImageRequestHandler::initWithData_options(VNImageRequestHandler::alloc(), &data, &options);
        let as_request: &VNRequest = &request;
        handler
            .performRequests_error(&NSArray::from_slice(&[as_request]))
            .map_err(|e| format!("Text recognition failed ({}).", e.localizedDescription()))?;

        let Some(results) = request.results() else {
            return Ok(Vec::new());
        };
        let mut boxes = Vec::with_capacity(results.count());
        for observation in results.iter() {
            let Some(best) = observation.topCandidates(1).firstObject() else {
                continue;
            };
            // Normalised coordinates with the origin at the bottom-left.
            let r = unsafe { observation.boundingBox() };
            boxes.push(TextBox {
                x: (r.origin.x * w) as f32,
                y: ((1.0 - r.origin.y - r.size.height) * h) as f32,
                width: (r.size.width * w) as f32,
                height: (r.size.height * h) as f32,
                text: best.string().to_string(),
            });
        }
        Ok(boxes)
    })
}

/// The user's preferred languages that Vision can read, in preference order,
/// with English as a fallback. `None` keeps Vision's default.
fn recognition_languages(request: &VNRecognizeTextRequest) -> Option<objc2::rc::Retained<NSArray<NSString>>> {
    let supported: Vec<String> = unsafe { request.supportedRecognitionLanguagesAndReturnError() }
        .ok()?
        .iter()
        .map(|s| s.to_string())
        .collect();
    let preferred: Vec<String> = NSLocale::preferredLanguages()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut chosen: Vec<&str> = Vec::new();
    for p in preferred.iter().map(String::as_str).chain(["en-US"]) {
        let primary = p.split('-').next().unwrap_or(p);
        for s in &supported {
            let matches = s == p
                || p.starts_with(&format!("{s}-"))
                || s.split('-').next() == Some(primary);
            if matches && !chosen.contains(&s.as_str()) {
                chosen.push(s);
            }
        }
    }
    if chosen.is_empty() {
        return None;
    }
    let strings: Vec<_> = chosen.into_iter().map(NSString::from_str).collect();
    Some(NSArray::from_retained_slice(&strings))
}
