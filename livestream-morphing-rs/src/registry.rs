pub use crate::effect_types::*;

/// Factory for creating effect instances. Used by the registration macro.
pub struct EffectFactory(pub fn() -> Box<dyn Effect>);

inventory::collect!(EffectFactory);

/// Return a fresh instance of every registered effect.
pub fn all_effects() -> Vec<Box<dyn Effect>> {
    inventory::iter::<EffectFactory>
        .into_iter()
        .map(|f| (f.0)())
        .collect()
}

/// Build a ParamValues map with defaults from the given ParamDefs.
pub fn default_params(defs: &[ParamDef]) -> ParamValues {
    defs.iter().map(|p| (p.id.to_string(), p.default)).collect()
}

/// Registration macro. Place at the bottom of each effect file.
/// The effect type must implement Default.
#[macro_export]
macro_rules! register_effect {
    ($ty:ty) => {
        inventory::submit! {
            $crate::registry::EffectFactory(|| Box::new(<$ty>::default()))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_def_new() {
        let p = ParamDef::new("radius", "Blur Radius", 0.0, 20.0, 3.0, 0.5);
        assert_eq!(p.id, "radius");
        assert_eq!(p.min, 0.0);
        assert_eq!(p.max, 20.0);
        assert_eq!(p.default, 3.0);
        assert_eq!(p.step, 0.5);
    }

    #[test]
    fn default_params_builds_map() {
        let defs = vec![
            ParamDef::new("a", "A", 0.0, 1.0, 0.5, 0.1),
            ParamDef::new("b", "B", 0.0, 10.0, 5.0, 1.0),
        ];
        let vals = default_params(&defs);
        assert_eq!(vals.get("a"), Some(&0.5));
        assert_eq!(vals.get("b"), Some(&5.0));
    }

    #[test]
    fn param_def_serializes_to_json() {
        let p = ParamDef::new("test", "Test", 0.0, 1.0, 0.5, 0.1);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"id\":\"test\""));
        assert!(json.contains("\"default\":0.5"));
    }
}
