use sc_ocr::OcrCompletionData;

pub enum HostEvent {
    OcrAvailabilityChanged { available: bool },
    OcrCompleted(OcrCompletionData),
    OcrCancelled,
}

impl std::fmt::Debug for HostEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostEvent::OcrAvailabilityChanged { available } => f
                .debug_struct("OcrAvailabilityChanged")
                .field("available", available)
                .finish(),
            HostEvent::OcrCompleted(_) => f.write_str("OcrCompleted(..)"),
            HostEvent::OcrCancelled => f.write_str("OcrCancelled"),
        }
    }
}
