use ::windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GLYPH_METRICS, DWriteCreateFactory, IDWriteFactory,
    IDWriteFontFamily, IDWriteLocalizedStrings,
};
use ::windows::core::{BOOL, w};

/// Installed DirectWrite families whose printable ASCII glyphs share one advance.
pub fn installed_families() -> Vec<String> {
    unsafe { families() }.unwrap_or_default()
}

unsafe fn families() -> ::windows::core::Result<Vec<String>> {
    let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let mut collection = None;
    unsafe { factory.GetSystemFontCollection(&mut collection, false) }?;
    let Some(collection) = collection else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for index in 0..unsafe { collection.GetFontFamilyCount() } {
        let Ok(family) = (unsafe { collection.GetFontFamily(index) }) else {
            continue;
        };
        if unsafe { monospace(&family) }
            && let Ok(strings) = unsafe { family.GetFamilyNames() }
            && let Some(name) = unsafe { family_name(&strings) }
        {
            names.push(name);
        }
    }
    Ok(names)
}

unsafe fn monospace(family: &IDWriteFontFamily) -> bool {
    let Ok(font) = (unsafe {
        family.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )
    }) else {
        return false;
    };
    if unsafe { font.IsSymbolFont() }.as_bool() {
        return false;
    }
    let Ok(face) = (unsafe { font.CreateFontFace() }) else {
        return false;
    };
    let codepoints: Vec<u32> = (0x21..=0x7e).collect();
    let mut glyphs = vec![0u16; codepoints.len()];
    if unsafe {
        face.GetGlyphIndices(
            codepoints.as_ptr(),
            codepoints.len() as u32,
            glyphs.as_mut_ptr(),
        )
    }
    .is_err()
        || glyphs.contains(&0)
    {
        return false;
    }
    let mut metrics = vec![DWRITE_GLYPH_METRICS::default(); glyphs.len()];
    if unsafe {
        face.GetDesignGlyphMetrics(
            glyphs.as_ptr(),
            glyphs.len() as u32,
            metrics.as_mut_ptr(),
            false,
        )
    }
    .is_err()
    {
        return false;
    }
    super::same_advance(metrics.iter().map(|m| m.advanceWidth as f64))
}

/// The en-US family name, which `FindFamilyName` resolves in every UI language.
unsafe fn family_name(strings: &IDWriteLocalizedStrings) -> Option<String> {
    let mut index = 0;
    let mut exists = BOOL::default();
    let found = unsafe { strings.FindLocaleName(w!("en-us"), &mut index, &mut exists) }.is_ok()
        && exists.as_bool();
    if !found {
        if unsafe { strings.GetCount() } == 0 {
            return None;
        }
        index = 0;
    }
    let length = unsafe { strings.GetStringLength(index) }.ok()? as usize;
    let mut buffer = vec![0u16; length + 1];
    unsafe { strings.GetString(index, &mut buffer) }.ok()?;
    let name = String::from_utf16(&buffer[..length]).ok()?;
    (!name.is_empty()).then_some(name)
}
