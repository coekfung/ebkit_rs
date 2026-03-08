//! Shared types for the ebkit workspace.

use std::collections::BTreeMap;

/// Matches the HDF5 compound type `{x: u16, y: u16, p: i16, t: i64}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCD {
    pub x: u16,
    pub y: u16,
    /// 0 = brightness decrease (OFF), 1 = brightness increase (ON).
    pub p: i16,
    /// Microseconds.
    pub t: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtTrigger {
    /// 0 = falling, 1 = rising.
    pub p: i16,
    /// Channel ID (e.g., 0 = EXTTRIG, 1 = TDRSTN/PXRSTN).
    pub c: i16,
    /// Microseconds.
    pub t: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventEncoding {
    Evt20,
    Evt21,
    Evt30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub encoding: Option<EventEncoding>,
    pub geometry: Geometry,
    pub camera_integrator_name: Option<String>,
    pub plugin_integrator_name: Option<String>,
    pub plugin_name: Option<String>,
    pub serial_number: Option<String>,
    pub system_id: Option<String>,
    /// `YYYY-MM-DD HH:MM:SS`
    pub date: Option<String>,
    /// e.g. `"4.2"`
    pub generation: Option<String>,
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub metadata: Metadata,
    /// In timestamp order.
    pub events: Vec<EventCD>,
    /// In timestamp order.
    pub triggers: Vec<ExtTrigger>,
}
