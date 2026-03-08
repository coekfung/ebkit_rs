use ebkit_core::{EventCD, ExtTrigger};

pub(crate) mod evt20;

pub trait EvtDecoder {
    fn decode(&mut self, buf: &[u8]) -> (Vec<EventCD>, Vec<ExtTrigger>);
}
