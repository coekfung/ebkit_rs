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
pub struct MetadataBuilder {
    encoding: Option<EventEncoding>,
    geometry: Option<Geometry>,
    camera_integrator_name: Option<String>,
    plugin_integrator_name: Option<String>,
    plugin_name: Option<String>,
    serial_number: Option<String>,
    system_id: Option<String>,
    date: Option<String>,
    generation: Option<String>,
    extra: BTreeMap<String, String>,
}

impl Default for MetadataBuilder {
    fn default() -> Self {
        Self {
            encoding: None,
            geometry: None,
            camera_integrator_name: None,
            plugin_integrator_name: None,
            plugin_name: None,
            serial_number: None,
            system_id: None,
            date: None,
            generation: None,
            extra: BTreeMap::new(),
        }
    }
}

impl MetadataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encoding(&self) -> Option<EventEncoding> {
        self.encoding
    }

    pub fn geometry(&self) -> Option<Geometry> {
        self.geometry
    }

    pub fn generation(&self) -> Option<&str> {
        self.generation.as_deref()
    }

    pub fn camera_integrator_name(&self) -> Option<&str> {
        self.camera_integrator_name.as_deref()
    }

    pub fn with_encoding(mut self, encoding: EventEncoding) -> Self {
        self.encoding = Some(encoding);
        return self;
    }

    pub fn with_geometry(mut self, geometry: Geometry) -> Self {
        self.geometry = Some(geometry);
        self
    }

    pub fn with_camera_integrator_name(mut self, value: String) -> Self {
        self.camera_integrator_name = Some(value);
        self
    }

    pub fn with_plugin_integrator_name(mut self, value: String) -> Self {
        self.plugin_integrator_name = Some(value);
        self
    }

    pub fn with_plugin_name(mut self, value: String) -> Self {
        self.plugin_name = Some(value);
        self
    }

    pub fn with_serial_number(mut self, value: String) -> Self {
        self.serial_number = Some(value);
        self
    }

    pub fn with_system_id(mut self, value: String) -> Self {
        self.system_id = Some(value);
        self
    }

    pub fn with_date(mut self, value: String) -> Self {
        self.date = Some(value);
        self
    }

    pub fn with_generation(mut self, value: String) -> Self {
        self.generation = Some(value);
        self
    }

    pub fn with_extra(mut self, key: String, value: String) -> Self {
        self.extra.insert(key, value);
        self
    }

    pub fn build(self) -> Option<Metadata> {
        let encoding = self.encoding?;
        let geometry = self.geometry?;

        Some(Metadata {
            encoding: Some(encoding),
            geometry,
            camera_integrator_name: self.camera_integrator_name,
            plugin_integrator_name: self.plugin_integrator_name,
            plugin_name: self.plugin_name,
            serial_number: self.serial_number,
            system_id: self.system_id,
            date: self.date,
            generation: self.generation,
            extra: self.extra,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub metadata: Metadata,
    /// In timestamp order.
    pub events: Vec<EventCD>,
    /// In timestamp order.
    pub triggers: Vec<ExtTrigger>,
}
