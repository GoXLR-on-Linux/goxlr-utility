use anyhow::Result;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Write;

pub struct PresetWriter {
    name: String,
}

impl PresetWriter {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn write_initial<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let formatted_name = self.name.replace(' ', "_");
        let mut elem = BytesStart::new(formatted_name.as_str());

        elem.push_attribute(("name", self.name.as_str()));
        writer.write_event(Event::Start(elem))?;
        Ok(())
    }

    pub fn write_tag<W: Write>(
        &self,
        writer: &mut Writer<W>,
        name: &str,
        attribute_map: HashMap<String, String>,
    ) -> Result<()> {
        let mut elem = BytesStart::new(name);
        for (key, value) in &attribute_map {
            elem.push_attribute((key.as_str(), value.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn write_final<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let formatted_name = self.name.replace(' ', "_");
        writer.write_event(Event::End(BytesEnd::new(formatted_name.as_str())))?;
        Ok(())
    }
}
