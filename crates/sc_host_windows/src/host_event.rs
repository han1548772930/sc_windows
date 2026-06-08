use sc_ocr::OcrCompletionData;

pub enum HostEvent {
    OcrAvailabilityChanged {
        generation: u64,
        available: bool,
    },
    OcrCompleted {
        generation: u64,
        data: OcrCompletionData,
    },
    OcrCancelled {
        generation: u64,
    },
}

impl std::fmt::Debug for HostEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostEvent::OcrAvailabilityChanged {
                generation,
                available,
            } => f
                .debug_struct("OcrAvailabilityChanged")
                .field("generation", generation)
                .field("available", available)
                .finish(),
            HostEvent::OcrCompleted { generation, .. } => f
                .debug_struct("OcrCompleted")
                .field("generation", generation)
                .finish_non_exhaustive(),
            HostEvent::OcrCancelled { generation } => f
                .debug_struct("OcrCancelled")
                .field("generation", generation)
                .finish(),
        }
    }
}
