use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub enum Kind {
    Boolean,
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    Range {
        min: f64,
        max: f64,
        step: f64,
    },
    Choice(Vec<String>),
    Color,
    Text {
        max_chars: usize,
    },
    StringList {
        max_entries: usize,
        max_chars: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Condition {
    pub setting: String,
    pub expected: Value,
    pub negate: bool,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: Kind,
    pub default: Value,
    pub visible_if: Vec<Condition>,
}

impl Definition {
    pub fn new(id: &str, label: &str, kind: Kind, default: Value) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            kind,
            default,
            visible_if: Vec::new(),
        }
    }
    pub fn boolean(id: &str, label: &str, default: bool) -> Self {
        Self::new(id, label, Kind::Boolean, json!(default))
    }
    pub fn number(id: &str, label: &str, min: f64, default: f64, max: f64, step: f64) -> Self {
        Self::new(id, label, Kind::Number { min, max, step }, json!(default))
    }
    pub fn choice(id: &str, label: &str, choices: &[&str], default: &str) -> Self {
        Self::new(
            id,
            label,
            Kind::Choice(choices.iter().map(|v| (*v).into()).collect()),
            json!(default),
        )
    }
    pub fn when(mut self, id: &str, expected: Value) -> Self {
        self.visible_if.push(Condition {
            setting: id.into(),
            expected,
            negate: false,
        });
        self
    }
    /// Shown only while `id` differs from `expected` (the reference's
    /// `getDisabledCondition().applyTo`).
    pub fn unless(mut self, id: &str, expected: Value) -> Self {
        self.visible_if.push(Condition {
            setting: id.into(),
            expected,
            negate: true,
        });
        self
    }
    pub fn validate(&self, value: &Value) -> Result<Value, String> {
        let bad = || format!("{}: invalid value for {}", self.id, self.label);
        let number = |v: &Value, min: f64, max: f64, step: f64| -> Result<f64, String> {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(bad)?;
            if !min.is_finite() || !max.is_finite() || min > max || !step.is_finite() || step <= 0.0
            {
                return Err(format!("{}: invalid numeric definition", self.id));
            }
            let clamped = n.clamp(min, max);
            // Reference values round to an increment around zero, not the minimum.
            Ok(((clamped / step + 0.5).floor() * step).clamp(min, max))
        };
        match &self.kind {
            Kind::Boolean if value.is_boolean() => Ok(value.clone()),
            Kind::Number { min, max, step } => Ok(json!(number(value, *min, *max, *step)?)),
            Kind::Range { min, max, step } => {
                let values = value.as_array().filter(|a| a.len() == 2).ok_or_else(bad)?;
                let lo = number(&values[0], *min, *max, *step)?;
                let hi = number(&values[1], *min, *max, *step)?;
                if lo > hi {
                    return Err(format!("{}: range minimum exceeds maximum", self.id));
                }
                Ok(json!([lo, hi]))
            }
            Kind::Choice(options)
                if value
                    .as_str()
                    .is_some_and(|v| options.iter().any(|s| s == v)) =>
            {
                Ok(value.clone())
            }
            Kind::Color if value.as_u64().is_some_and(|c| c <= u32::MAX as u64) => {
                Ok(value.clone())
            }
            Kind::Text { max_chars }
                if value
                    .as_str()
                    .is_some_and(|s| s.chars().count() <= *max_chars && !s.contains('\0')) =>
            {
                Ok(value.clone())
            }
            Kind::StringList {
                max_entries,
                max_chars,
            } => {
                let values = value
                    .as_array()
                    .filter(|a| a.len() <= *max_entries)
                    .ok_or_else(bad)?;
                let mut seen = BTreeSet::new();
                let mut result = Vec::new();
                for v in values {
                    let s = v
                        .as_str()
                        .filter(|s| s.chars().count() <= *max_chars && !s.contains('\0'))
                        .ok_or_else(bad)?;
                    if seen.insert(s) {
                        result.push(json!(s));
                    }
                }
                Ok(Value::Array(result))
            }
            _ => Err(bad()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    definitions: BTreeMap<String, Definition>,
    values: Map<String, Value>,
    unknown: Map<String, Value>,
}

impl Settings {
    pub fn define(&mut self, mut definition: Definition) -> Result<(), String> {
        if !valid_id(&definition.id) {
            return Err(format!("invalid setting ID: {}", definition.id));
        }
        if self.definitions.contains_key(&definition.id) {
            return Err(format!("duplicate setting ID: {}", definition.id));
        }
        let default = definition.validate(&definition.default)?;
        definition.default = default.clone();
        // Every reference `ColorValue` carries a "<name> rainbow" boolean.
        // It shares the colour's visibility, so a hidden colour never cycles.
        let rainbow = matches!(definition.kind, Kind::Color).then(|| Definition {
            visible_if: definition.visible_if.clone(),
            ..Definition::boolean(
                &format!("{}.rainbow", definition.id),
                &format!("{} rainbow", definition.label),
                false,
            )
        });
        self.values.insert(definition.id.clone(), default);
        self.definitions.insert(definition.id.clone(), definition);
        match rainbow {
            Some(rainbow) => self.define(rainbow),
            None => Ok(()),
        }
    }
    /// The colour a `<id>.rainbow` sibling belongs to.
    pub fn rainbow_of<'a>(&self, id: &'a str) -> Option<&'a str> {
        let color = id.strip_suffix(".rainbow")?;
        matches!(self.definitions.get(color)?.kind, Kind::Color).then_some(color)
    }
    /// Colours whose rainbow sibling is on.
    pub fn rainbow_colors(&self) -> impl Iterator<Item = &str> {
        self.definitions
            .values()
            .filter(|d| matches!(d.kind, Kind::Color))
            .map(|d| d.id.as_str())
            .filter(|id| {
                self.effective(&format!("{id}.rainbow"))
                    .is_some_and(|v| v == true)
            })
    }
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.values()
    }
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.values.get(id)
    }
    pub fn validate_value(&self, id: &str, value: &Value) -> Result<Value, String> {
        self.definitions
            .get(id)
            .ok_or_else(|| format!("unknown setting: {id}"))?
            .validate(value)
    }
    pub fn set(&mut self, id: &str, value: Value) -> Result<bool, String> {
        let definition = self
            .definitions
            .get(id)
            .ok_or_else(|| format!("unknown setting: {id}"))?;
        let value = definition.validate(&value)?;
        if self.values.get(id) == Some(&value) {
            return Ok(false);
        }
        self.values.insert(id.into(), value);
        Ok(true)
    }
    pub fn visible(&self, id: &str) -> bool {
        self.definitions.get(id).is_some_and(|d| {
            d.visible_if.iter().all(|c| {
                self.get(&c.setting)
                    .is_some_and(|v| (v == &c.expected) != c.negate)
            })
        })
    }
    pub fn effective(&self, id: &str) -> Option<Value> {
        let d = self.definitions.get(id)?;
        // A hidden boolean evaluates false, while its user's raw value is kept.
        if matches!(d.kind, Kind::Boolean) && !self.visible(id) {
            Some(json!(false))
        } else {
            self.get(id).cloned()
        }
    }
    pub fn known_values(&self) -> Value {
        Value::Object(self.values.clone())
    }
    pub fn snapshot(&self) -> Value {
        let mut values = self.unknown.clone();
        values.extend(self.values.clone());
        Value::Object(values)
    }
    pub fn load(&mut self, document: &Value) -> Vec<String> {
        let Some(values) = document.as_object() else {
            return vec!["settings must be an object".into()];
        };
        let mut warnings = Vec::new();
        self.unknown.clear();
        for d in self.definitions.values() {
            self.values.insert(d.id.clone(), d.default.clone());
        }
        for (id, value) in values {
            if self.definitions.contains_key(id) {
                match self.set(id, value.clone()) {
                    Err(error) => warnings.push(error),
                    Ok(_) => {
                        if !equivalent(self.get(id).unwrap(), value) {
                            warnings
                                .push(format!("{id}: adjusted value to valid bounds/increments"));
                        }
                    }
                }
            } else {
                self.unknown.insert(id.clone(), value.clone());
            }
        }
        warnings
    }
    pub fn validate_conditions(&self) -> Result<(), String> {
        fn visit(
            settings: &Settings,
            id: &str,
            active: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
        ) -> Result<(), String> {
            if done.contains(id) {
                return Ok(());
            }
            if !active.insert(id.into()) {
                return Err(format!("cyclic visibility at {id}"));
            }
            let d = settings
                .definitions
                .get(id)
                .ok_or_else(|| format!("unknown visibility dependency: {id}"))?;
            for c in &d.visible_if {
                visit(settings, &c.setting, active, done)?;
            }
            active.remove(id);
            done.insert(id.into());
            Ok(())
        }
        let mut done = BTreeSet::new();
        for id in self.definitions.keys() {
            visit(self, id, &mut BTreeSet::new(), &mut done)?;
        }
        Ok(())
    }
}

fn equivalent(a: &Value, b: &Value) -> bool {
    if a.is_number() && b.is_number() {
        return a.as_f64() == b.as_f64();
    }
    match (a.as_array(), b.as_array()) {
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b)),
        _ => a == b,
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_edits_clamp_and_quantize_and_invalid_import_keeps_default() {
        let mut s = Settings::default();
        s.define(Definition::number("reach", "Reach", 0.0, 3.0, 6.0, 0.1))
            .unwrap();
        s.set("reach", json!(100)).unwrap();
        assert_eq!(s.get("reach"), Some(&json!(6.0)));
        s.set("reach", json!(3.13)).unwrap();
        assert!((s.get("reach").unwrap().as_f64().unwrap() - 3.1).abs() < 1e-10);
        assert_eq!(s.load(&json!({"reach":"NaN"})).len(), 1);
        assert_eq!(s.get("reach"), Some(&json!(3.0)));
    }
    #[test]
    fn hidden_boolean_is_false_but_preserves_raw_preference() {
        let mut s = Settings::default();
        s.define(Definition::choice("mode", "Mode", &["PID", "AI"], "PID"))
            .unwrap();
        s.define(Definition::boolean("adaptive", "Adaptive", true).when("mode", json!("AI")))
            .unwrap();
        s.validate_conditions().unwrap();
        assert_eq!(s.effective("adaptive"), Some(json!(false)));
        s.set("mode", json!("AI")).unwrap();
        assert_eq!(s.effective("adaptive"), Some(json!(true)));
        s.set("mode", json!("PID")).unwrap();
        assert_eq!(s.get("adaptive"), Some(&json!(true)));
    }
    #[test]
    fn unknown_values_round_trip_and_bad_choice_does_not_replace_default() {
        let mut s = Settings::default();
        s.define(Definition::choice(
            "layout",
            "Layout",
            &["horizontal", "vertical"],
            "horizontal",
        ))
        .unwrap();
        assert_eq!(
            s.load(&json!({"layout":"diagonal","future":{"enabled":true}}))
                .len(),
            1
        );
        assert_eq!(
            s.snapshot(),
            json!({"layout":"horizontal","future":{"enabled":true}})
        );
    }
    #[test]
    fn ranges_colors_and_text_are_validated() {
        let r = Definition::new(
            "cps",
            "CPS",
            Kind::Range {
                min: 1.0,
                max: 20.0,
                step: 0.1,
            },
            json!([8, 12]),
        );
        assert!(r.validate(&json!([14, 10])).is_err());
        assert!(r.validate(&json!([8, 12, 14])).is_err());
        assert_eq!(r.validate(&json!([0, 99])).unwrap(), json!([1.0, 20.0]));
        let c = Definition::new("color", "Color", Kind::Color, json!(0xffffffffu32));
        assert!(c.validate(&json!(-1)).is_err());
        assert!(c.validate(&json!(4294967296u64)).is_err());
        let t = Definition::new("alias", "Alias", Kind::Text { max_chars: 2 }, json!(""));
        assert!(t.validate(&json!("你好")).is_ok());
        assert!(t.validate(&json!("abc")).is_err());
    }
    #[test]
    fn schema_rejects_duplicate_ids_and_dependency_cycles() {
        let mut s = Settings::default();
        s.define(Definition::boolean("a", "A", false).when("b", json!(true)))
            .unwrap();
        assert!(s.validate_conditions().is_err());
        s.define(Definition::boolean("b", "B", false).when("a", json!(true)))
            .unwrap();
        assert!(s.validate_conditions().is_err());
        assert!(s.define(Definition::boolean("a", "A", true)).is_err());
    }
}
