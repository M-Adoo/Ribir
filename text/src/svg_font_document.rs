use log::warn;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use rustybuzz::ttf_parser::GlyphId;

use ahash::{HashMap, HashSet};
use std::borrow::Cow;

use std::io::prelude::*;

use crate::font_db::Face;

pub(crate) struct SvgDocument {
  elems: HashMap<String, String>,
}

impl SvgDocument {
  pub(crate) fn parse(content: &str) -> Option<Self> {
    let mut reader = Reader::from_str(content);
    let mut buf = Vec::new();
    let mut doc = Self { elems: HashMap::default() };
    loop {
      match reader.read_event_into(&mut buf) {
        Ok(ref e @ Event::Start(ref tag)) | Ok(ref e @ Event::Empty(ref tag)) => {
          if tag.name() != QName(b"defs") {
            let has_child = matches!(e, Event::Start(_));
            doc.collect_named_obj(&mut reader, content, tag, has_child);
          }
        }
        Ok(Event::Eof) => break, // exits the loop when reaching end of file
        Err(e) => {
          warn!("Error at position {}: {:?}", reader.buffer_position(), e);
          return None;
        }

        _ => (), // There are several other `Event`s we do not consider here
      }
    }
    Some(doc)
  }

  pub fn glyph_svg(&self, glyph: GlyphId, face: &Face) -> Option<String> {
    let key = format!("glyph{}", glyph.0);
    if !self.elems.contains_key(&key) {
      return None;
    }

    let mut all_links = HashSet::default();
    let mut elems = vec![key.clone()];

    while let Some(curr) = elems.pop() {
      if let Some(content) = self.elems.get(&curr) {
        elems.extend(Self::collect_link(content, &mut all_links));
      }
    }

    let units_per_em = face.units_per_em() as i32;
    let ascender = face.rb_face.ascender() as i32;
    let mut writer = std::io::Cursor::new(Vec::new());

    writer.write_all(format!(
      "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.1\" width=\"{}\" height=\"{}\" viewBox=\"{},{},{},{}\">",
       units_per_em, units_per_em,
       0, -ascender, units_per_em, units_per_em
      ).as_bytes()).ok()?;
    writer.write_all("<defs>".as_bytes()).ok()?;
    for link in all_links {
      if let Some(content) = self.elems.get(&link) {
        writer.write_all(content.as_bytes()).ok()?;
      }
    }
    writer.write_all("</defs>".as_bytes()).ok()?;
    writer
      .write_all(self.elems.get(&key).unwrap().as_bytes())
      .ok()?;
    writer.write_all("</svg>".as_bytes()).ok()?;

    Some(
      std::str::from_utf8(&writer.into_inner())
        .unwrap()
        .to_string(),
    )
  }

  fn collect_named_obj(
    &mut self,
    reader: &mut Reader<&[u8]>,
    source: &str,
    e: &BytesStart,
    has_children: bool,
  ) {
    if let Some(id) = e
      .attributes()
      .find(|a| a.as_ref().map_or(false, |a| a.key == QName(b"id")))
      .map(|a| a.unwrap().value)
    {
      unsafe {
        let content = Self::extra_elem(reader, e, source, has_children);
        self
          .elems
          .insert(std::str::from_utf8_unchecked(&id).to_string(), content);
      }
    };
  }

  unsafe fn extra_elem(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart,
    source: &str,
    has_children: bool,
  ) -> String {
    let content = if has_children {
      let mut buf = Vec::new();
      let rg = reader
        .read_to_end_into(e.name().to_owned(), &mut buf)
        .unwrap();
      &source[rg.start..rg.end]
    } else {
      ""
    };

    let name = e.name();
    let name = reader.decoder().decode(name.as_ref()).unwrap();

    format!(
      "<{}>{}</{}>",
      std::str::from_utf8_unchecked(e),
      content,
      name
    )
  }

  fn collect_link(content: &str, all_links: &mut HashSet<String>) -> Vec<String> {
    let mut reader = Reader::from_str(content);
    let mut buf = Vec::new();
    let mut new_links = Vec::new();
    loop {
      match reader.read_event_into(&mut buf) {
        Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
          Self::collect_link_from_attrs(e, all_links, &mut new_links);
        }
        Ok(Event::Eof) => break, // exits the loop when reaching end of file
        Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),

        _ => (), // There are several other `Event`s we do not consider here
      }
    }
    new_links
  }

  #[inline]
  fn extra_link_from_iri_func(val: Cow<'_, [u8]>) -> Option<String> {
    let val: &str = std::str::from_utf8(&val)
      .unwrap()
      .trim()
      .strip_prefix("url(")?
      .trim_start()
      .strip_prefix('#')?
      .strip_suffix(')')?;
    Some(val.to_string())
  }

  #[inline]
  fn extra_link_from_href(attr: &Attribute) -> Option<String> {
    if attr.key == QName(b"xlink:href") || attr.key == QName(b"href") {
      let href = std::str::from_utf8(&attr.value).unwrap();
      return Some(href.trim().strip_prefix('#')?.to_string());
    }
    None
  }

  fn collect_link_from_attrs(
    elem: &BytesStart,
    all_links: &mut HashSet<String>,
    new_links: &mut Vec<String>,
  ) {
    let attributes = elem.attributes();

    attributes.for_each(|attr| {
      let attr = attr.unwrap();
      if let Some(link) =
        Self::extra_link_from_href(&attr).or_else(|| Self::extra_link_from_iri_func(attr.value))
      {
        if all_links.contains(&link) {
          return;
        }
        all_links.insert(link.clone());
        new_links.push(link);
      }
    });
  }
}

#[cfg(test)]
mod tests {
  use rustybuzz::ttf_parser::GlyphId;

  use crate::font_db::FontDB;

  #[test]
  fn test_svg_document() {
    let content = r##"
        <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1">
          <defs>
            <path
              d="M262,-672 Q222,-610 216,-563 Q210,-516 237,-500 Q250,-493 262,-501 Q274,-509 284.5,-525 Q295,-541 303.5,-558.5 Q312,-576 319,-586 Q399,-705 535,-749 Q545,-753 556.5,-758 Q568,-763 573,-773 Q579,-785 572.5,-794.5 Q566,-804 554,-808 Q540,-814 522.5,-813.5 Q505,-813 488,-810 Q417,-798 355,-759.5 Q293,-721 262,-672 Z"
              id="u1F250.2"></path>
            <path
              d="M393,25 Q393,-4 372.5,-24 Q352,-44 324,-44 Q296,-44 276,-24 Q256,-4 256,25 Q256,53 276,73 Q296,93 324,93 Q352,93 372.5,73 Q393,53 393,25 Z"
              id="u1F69E.17"></path>
            <radialGradient id="g799" cx="638" cy="380" r="508" gradientUnits="userSpaceOnUse"
              gradientTransform="matrix(1 0 0 0.525 0 0)">
              <stop offset="0.598" stop-color="#212121" />
              <stop offset="1" stop-color="#616161" />
            </radialGradient>
          </defs>
          <g id="glyph2428">
            <use xlink:href="#u1F69E.17" x="-1886.951" y="-548.858"
              transform="matrix(7.674 0 0 7.674 12593.511 3663.078)" fill="#FFCC32" />
          </g>
        </svg>"##;
    let doc = super::SvgDocument::parse(content).unwrap();
    let mut db = FontDB::default();
    let dummy_face = db.face_data_or_insert(db.default_font()).unwrap();
    assert_eq!(doc.elems.len(), 4);
    assert!(doc.glyph_svg(GlyphId(2428), dummy_face).is_some());
    assert!(doc.glyph_svg(GlyphId(0), dummy_face).is_none());
  }
}