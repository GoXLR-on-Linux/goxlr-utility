use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;
use xml::writer::XmlEvent;
use xml::EventWriter;

pub struct PresetWriter {
    name: String,
}

impl PresetWriter {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn write_initial<W: Write>(&self, writer: &mut EventWriter<&mut W>) -> Result<()> {
        let formatted_name = self.name.replace(' ', "_");

        let mut root = XmlEvent::start_element(formatted_name.as_str());
        root = root.attr("name", self.name.as_str());
        writer.write(root)?;

        Ok(())
    }

    pub fn write_tag<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
        name: &str,
        attribute_map: HashMap<String, String>,
    ) -> Result<()> {
        let mut element = XmlEvent::start_element(name);

        for (key, value) in &attribute_map {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        Ok(())
    }

    pub fn write_final<W: Write>(&self, writer: &mut EventWriter<&mut W>) -> Result<()> {
        writer.write(XmlEvent::end_element())?;
        Ok(())
    }
}
