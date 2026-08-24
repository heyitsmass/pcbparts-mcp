pub mod values;
pub mod models;
pub mod packages;
pub mod connectors;
pub mod semantic;
pub mod types;
pub mod mapping;
pub mod parser;

pub use connectors::{extract_connector_series, get_pitch_for_series, ConnectorSpec};
pub use mapping::{category_attribute_map, infer_subcategory_from_values, map_value_to_spec};
pub use models::extract_model_number;
pub use packages::extract_package;
pub use parser::{merge_spec_filters, parse_smart_query, ParsedQuery};
pub use semantic::{connector_noise_words, extract_semantic_descriptors, noise_words, remove_noise_words, SemanticFilter};
pub use types::{extract_component_type, extract_mounting_type};
pub use values::{extract_values, ExtractedValue};
