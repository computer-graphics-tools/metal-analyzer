use dashmap::DashMap;

use crate::symbols::types::SymbolLocation;

pub struct SymbolIndex {
    pub(crate) map: DashMap<String, Vec<SymbolLocation>>,
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            map: DashMap::new(),
        }
    }

    pub fn insert(
        &self,
        name: String,
        loc: SymbolLocation,
    ) {
        self.map.entry(name).or_default().push(loc);
    }

    pub fn get(
        &self,
        name: &str,
    ) -> Vec<SymbolLocation> {
        self.map.get(name).map(|v| v.clone()).unwrap_or_default()
    }

    /// Search for symbols matching a query (case-insensitive substring match).
    /// Returns up to `limit` results.
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<(String, SymbolLocation)> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for entry in self.map.iter() {
            let symbol_name = entry.key();
            if symbol_name.to_lowercase().contains(&query_lower) {
                for loc in entry.value() {
                    results.push((symbol_name.clone(), loc.clone()));
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }

        results
    }
}
