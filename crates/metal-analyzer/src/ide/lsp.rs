use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};

use crate::ide::navigation::{IdeLocation, IdePosition, IdeRange, NavigationTarget};

pub fn lsp_position_to_ide(position: Position) -> IdePosition {
    IdePosition::new(position.line, position.character)
}

pub fn ide_position_to_lsp(position: IdePosition) -> Position {
    Position::new(position.line, position.character)
}

pub fn lsp_range_to_ide(range: Range) -> IdeRange {
    IdeRange::new(lsp_position_to_ide(range.start), lsp_position_to_ide(range.end))
}

pub fn ide_range_to_lsp(range: IdeRange) -> Range {
    Range::new(ide_position_to_lsp(range.start), ide_position_to_lsp(range.end))
}

pub fn ide_location_to_lsp(location: IdeLocation) -> Option<Location> {
    let uri = Url::from_file_path(&location.file_path).ok()?;
    Some(Location {
        uri,
        range: ide_range_to_lsp(location.range),
    })
}

pub fn navigation_target_to_lsp(target: NavigationTarget) -> Option<GotoDefinitionResponse> {
    match target {
        NavigationTarget::Single(location) => ide_location_to_lsp(location).map(GotoDefinitionResponse::Scalar),
        NavigationTarget::Multiple(locations) => {
            let lsp_locations: Vec<Location> = locations.into_iter().filter_map(ide_location_to_lsp).collect();
            if lsp_locations.is_empty() {
                None
            } else if lsp_locations.len() == 1 {
                lsp_locations.into_iter().next().map(GotoDefinitionResponse::Scalar)
            } else {
                Some(GotoDefinitionResponse::Array(lsp_locations))
            }
        },
    }
}
