//! Helpers for the capture harness (`examples/ui_snapshots.rs`).

/// Parks the pointer off the window for this frame: nothing under it is hovered, so a hover
/// state left by the real pointer (or by an earlier shot's click) never reaches a capture.
pub fn park_pointer(raw: &mut egui::RawInput) {
    raw.events.insert(0, egui::Event::PointerGone);
}

/// A widget the capture harness can click by name: its accessible name (the label a screen reader
/// is told, `AccessKit`'s) and where it is on screen, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedRect {
    /// The widget's accessible name.
    pub name: String,
    /// The widget's clickable area.
    pub rect: egui::Rect,
}

/// Why [`find_named`] could not pick one widget.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FindError {
    /// No widget on screen has this accessible name. `known` lists the names that are there.
    #[error("no widget is named {name:?}; the names on screen are {known:?}")]
    NotFound {
        /// The name asked for.
        name: String,
        /// Every name drawn this frame, sorted.
        known: Vec<String>,
    },
    /// More than one widget has this accessible name, so a click could land on the wrong one.
    #[error("{count} widgets are named {name:?}; the click would be ambiguous")]
    Ambiguous {
        /// The name asked for.
        name: String,
        /// How many widgets carry it.
        count: usize,
    },
}

/// The named widgets drawn in the pass that is running, to be read at its end. Needs
/// [`egui::Context::enable_accesskit`] to have been called: without it no widget has a name and
/// the list is empty.
#[must_use]
pub fn collect_named(ctx: &egui::Context) -> Vec<NamedRect> {
    let widgets: Vec<(egui::Id, egui::Rect, bool)> = ctx.viewport(|viewport| {
        viewport
            .this_pass
            .widgets
            .layers()
            .flat_map(|(_, rects)| rects.iter())
            .map(|w| (w.id, w.interact_rect, w.enabled))
            .collect()
    });
    widgets
        .into_iter()
        .filter(|(id, rect, enabled)| *enabled && rect.is_positive() && ctx.has_accesskit_node(*id))
        .filter_map(|(id, rect, _)| {
            let name = ctx.accesskit_node_builder(id, |node| node.label().map(str::to_owned))??;
            Some(NamedRect { name, rect })
        })
        .collect()
}

/// The point near the left end, at the vertical middle, of the one widget named `name`: for a
/// widget whose middle another popup covers.
///
/// # Errors
/// As [`find_named`].
pub fn find_named_start(rects: &[NamedRect], name: &str) -> Result<egui::Pos2, FindError> {
    // The widget's own middle, then moved to its left end.
    let middle = find_named(rects, name)?;
    let width = rects
        .iter()
        .find(|r| r.name == name)
        .map_or(0.0, |r| r.rect.width());
    let inset = START_INSET.min(width / 2.0);
    Ok(egui::pos2(middle.x - width / 2.0 + inset, middle.y))
}

/// Points from a widget's left edge that [`find_named_start`] aims at.
const START_INSET: f32 = 12.0;

/// The point at the middle of the one widget named `name`.
///
/// # Errors
/// [`FindError::NotFound`] when none has that name and [`FindError::Ambiguous`] when several do.
pub fn find_named(rects: &[NamedRect], name: &str) -> Result<egui::Pos2, FindError> {
    let mut hits = rects.iter().filter(|r| r.name == name);
    match (hits.next(), hits.next()) {
        (Some(only), None) => Ok(only.rect.center()),
        (Some(_), Some(_)) => Err(FindError::Ambiguous {
            name: name.to_owned(),
            count: rects.iter().filter(|r| r.name == name).count(),
        }),
        (None, _) => {
            let mut known: Vec<String> = rects.iter().map(|r| r.name.clone()).collect();
            known.sort();
            known.dedup();
            Err(FindError::NotFound {
                name: name.to_owned(),
                known,
            })
        }
    }
}
