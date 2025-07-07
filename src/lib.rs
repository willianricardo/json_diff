//! # json_diff
//!
//! A lightweight Rust library for computing and applying JSON diffs. It allows you to compare two
//! `serde_json::Value`s, generate a delta of changes, apply those changes, and revert them if necessary.
//!
//! ## Example
//!
//! ```rust
//! use serde_json::json;
//! use json_diff::{diff, apply, revert, Change};
//!
//! let before = json!({ "a": 1, "b": { "c": true } });
//! let after  = json!({ "a": 2, "b": { "c": false }, "d": "new" });
//!
//! let delta = diff(&before, &after);
//! assert_eq!(delta.get("a"), Some(&Change::Modify { old: json!(1), new: json!(2) }));
//!
//! let applied = apply(&before, &delta);
//! assert_eq!(applied, after);
//!
//! let reverted = revert(&after, &delta);
//! assert_eq!(reverted, before);
//! ```

use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};

/// Represents a single JSON change.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// A value was added at the specified path.
    Add(Value),
    /// A value was removed from the specified path.
    Remove(Value),
    /// A value was modified: `old` → `new`.
    Modify { old: Value, new: Value },
}

impl Change {
    /// Returns the inverse of this change (adds ⇄ removes, swaps `old`/`new`).
    pub fn inverse(self) -> Self {
        match self {
            Change::Add(v) => Change::Remove(v),
            Change::Remove(v) => Change::Add(v),
            Change::Modify { old, new } => Change::Modify { old: new, new: old },
        }
    }
}

/// A mapping from JSON dot-paths to `Change` values.
pub type Delta = BTreeMap<String, Change>;

/// Compute the delta between two JSON values.
///
/// Returns a `Delta` mapping each changed path to its corresponding `Change`.
pub fn diff(before: &Value, after: &Value) -> Delta {
    let mut changes = Delta::new();
    compare(&mut changes, String::new(), before, after);
    changes
}

fn compare(delta: &mut Delta, path: String, a: &Value, b: &Value) {
    if a == b {
        return;
    }

    match (a, b) {
        (Value::Object(obj_a), Value::Object(obj_b)) => {
            // Collect all keys present in either object
            let all_keys: HashSet<_> = obj_a.keys().chain(obj_b.keys()).collect();
            for key in all_keys {
                let new_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };

                match (obj_a.get(key), obj_b.get(key)) {
                    (Some(va), Some(vb)) => compare(delta, new_path, va, vb),
                    (Some(va), None) => {
                        delta.insert(new_path, Change::Remove(va.clone()));
                    }
                    (None, Some(vb)) => {
                        delta.insert(new_path, Change::Add(vb.clone()));
                    }
                    _ => unreachable!(),
                }
            }
        }
        _ => {
            delta.insert(
                path,
                Change::Modify {
                    old: a.clone(),
                    new: b.clone(),
                },
            );
        }
    }
}

/// Apply a `Delta` to an original JSON value, returning a new `Value`.
pub fn apply(original: &Value, delta: &Delta) -> Value {
    let mut result = original.clone();
    for (path, change) in delta {
        let value = match change {
            Change::Add(v) | Change::Modify { new: v, .. } => Some(v.clone()),
            Change::Remove(_) => None,
        };
        set_value(&mut result, path, value);
    }
    result
}

/// Revert a `Delta` on a JSON value, returning the previous state.
pub fn revert(original: &Value, delta: &Delta) -> Value {
    let inverse_delta: Delta = delta
        .iter()
        .map(|(path, change)| (path.clone(), change.clone().inverse()))
        .collect();
    apply(original, &inverse_delta)
}

fn set_value(root: &mut Value, path: &str, value: Option<Value>) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;
    // Navigate to the parent of the target
    for &segment in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(segment)
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if let Some(obj) = current.as_object_mut() {
        let key = parts.last().unwrap();
        match value {
            Some(v) => {
                obj.insert(key.to_string(), v);
            }
            None => {
                obj.remove(*key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{E, PI};

    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn nested_user_profile_field_change() {
        let old_profile = json!({"name": "John", "preferences": {"theme": "dark"}});
        let new_profile = json!({"name": "John", "preferences": {"theme": "light"}});
        let delta: Delta = diff(&old_profile, &new_profile);

        let mut expected = Delta::new();
        expected.insert(
            "preferences.theme".to_string(),
            Change::Modify {
                old: json!("dark"),
                new: json!("light"),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&new_profile, &delta);
        assert_eq!(reverted, old_profile);

        let applied = apply(&old_profile, &delta);
        assert_eq!(applied, new_profile);
    }

    #[test]
    fn identical_objects_should_return_empty_delta() {
        let customer = json!({"name": "Mary", "address": {"city": "Curitiba"}});
        let delta: Delta = diff(&customer, &customer);
        assert_eq!(delta, Delta::new());

        let reverted = revert(&customer, &delta);
        assert_eq!(reverted, customer);

        let applied = apply(&customer, &delta);
        assert_eq!(applied, customer);
    }

    #[test]
    fn empty_objects_should_return_empty_delta() {
        let delta: Delta = diff(&json!({}), &json!({}));
        assert_eq!(delta, Delta::new());

        let reverted = revert(&json!({}), &delta);
        assert_eq!(reverted, json!({}));

        let applied = apply(&json!({}), &delta);
        assert_eq!(applied, json!({}));
    }

    #[test]
    fn removal_of_product_field() {
        let product_before = json!({"name": "Soap", "description": "Fragrant"});
        let product_after = json!({"name": "Soap"});
        let delta: Delta = diff(&product_before, &product_after);

        let mut expected = Delta::new();
        expected.insert("description".to_string(), Change::Remove(json!("Fragrant")));
        assert_eq!(delta, expected);

        let reverted = revert(&product_after, &delta);
        assert_eq!(reverted, product_before);

        let applied = apply(&product_before, &delta);
        assert_eq!(applied, product_after);
    }

    #[test]
    fn addition_of_product_field() {
        let product_before = json!({"name": "Soap"});
        let product_after = json!({"name": "Soap", "description": "Fragrant"});
        let delta: Delta = diff(&product_before, &product_after);

        let mut expected = Delta::new();
        expected.insert("description".to_string(), Change::Add(json!("Fragrant")));
        assert_eq!(delta, expected);

        let reverted = revert(&product_after, &delta);
        assert_eq!(reverted, product_before);

        let applied = apply(&product_before, &delta);
        assert_eq!(applied, product_after);
    }

    #[test]
    fn multiple_changes_in_order() {
        let order_before = json!({"quantity": 1, "status": "pending", "value": 100});
        let order_after = json!({"quantity": 1, "status": "shipped", "value": 110});
        let delta: Delta = diff(&order_before, &order_after);

        let mut expected = Delta::new();
        expected.insert(
            "status".to_string(),
            Change::Modify {
                old: json!("pending"),
                new: json!("shipped"),
            },
        );
        expected.insert(
            "value".to_string(),
            Change::Modify {
                old: json!(100),
                new: json!(110),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&order_after, &delta);
        assert_eq!(reverted, order_before);

        let applied = apply(&order_before, &delta);
        assert_eq!(applied, order_after);
    }

    #[test]
    fn nested_field_removal_in_address() {
        let address_before = json!({"location": {"street": "Main St"}});
        let address_after = json!({"location": {}});
        let delta: Delta = diff(&address_before, &address_after);

        let mut expected = Delta::new();
        expected.insert(
            "location.street".to_string(),
            Change::Remove(json!("Main St")),
        );
        assert_eq!(delta, expected);

        let reverted = revert(&address_after, &delta);
        assert_eq!(reverted, address_before);

        let applied = apply(&address_before, &delta);
        assert_eq!(applied, address_after);
    }

    #[test]
    fn nested_field_addition_in_address() {
        let address_before = json!({"location": {}});
        let address_after = json!({"location": {"street": "Main St"}});
        let delta: Delta = diff(&address_before, &address_after);

        let mut expected = Delta::new();
        expected.insert("location.street".to_string(), Change::Add(json!("Main St")));
        assert_eq!(delta, expected);

        let reverted = revert(&address_after, &delta);
        assert_eq!(reverted, address_before);

        let applied = apply(&address_before, &delta);
        assert_eq!(applied, address_after);
    }

    #[test]
    fn deep_config_changes() {
        let old_config = json!({"system": {"theme": {"color": {"primary": "blue"}}}});
        let new_config = json!({"system": {"theme": {"color": {"primary": "green"}}}});
        let delta: Delta = diff(&old_config, &new_config);

        let mut expected = Delta::new();
        expected.insert(
            "system.theme.color.primary".to_string(),
            Change::Modify {
                old: json!("blue"),
                new: json!("green"),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&new_config, &delta);
        assert_eq!(reverted, old_config);

        let applied = apply(&old_config, &delta);
        assert_eq!(applied, new_config);
    }

    #[test]
    fn array_value_change() {
        let before = json!({"numbers": [1, 2, 3]});
        let after = json!({"numbers": [1, 2, 4]});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "numbers".to_string(),
            Change::Modify {
                old: json!([1, 2, 3]),
                new: json!([1, 2, 4]),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn array_to_object_change() {
        let before = json!({"list": [1, 2, 3]});
        let after = json!({"list": {"0": 1, "1": 2, "2": 3}});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "list".to_string(),
            Change::Modify {
                old: json!([1, 2, 3]),
                new: json!({"0": 1, "1": 2, "2": 3}),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn null_and_undefined_values() {
        // Represent undefined as Null
        let before = json!({"a": null, "b": Value::Null, "c": "value"});
        let after = json!({"a": "not null", "b": "defined", "c": null});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "a".to_string(),
            Change::Modify {
                old: json!(null),
                new: json!("not null"),
            },
        );
        expected.insert(
            "b".to_string(),
            Change::Modify {
                old: json!(null),
                new: json!("defined"),
            },
        );
        expected.insert(
            "c".to_string(),
            Change::Modify {
                old: json!("value"),
                new: json!(null),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn boolean_changes() {
        let before = json!({"active": true, "verified": false});
        let after = json!({"active": false, "verified": true});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "active".to_string(),
            Change::Modify {
                old: json!(true),
                new: json!(false),
            },
        );
        expected.insert(
            "verified".to_string(),
            Change::Modify {
                old: json!(false),
                new: json!(true),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn positive_negative_zero_numbers() {
        let before = json!({"a": 0, "b": -5, "c": PI});
        let after = json!({"a": 1, "b": 0, "c": -PI});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "a".to_string(),
            Change::Modify {
                old: json!(0),
                new: json!(1),
            },
        );
        expected.insert(
            "b".to_string(),
            Change::Modify {
                old: json!(-5),
                new: json!(0),
            },
        );
        expected.insert(
            "c".to_string(),
            Change::Modify {
                old: json!(PI),
                new: json!(-PI),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn string_to_number_type_change() {
        let before = json!({"code": "123"});
        let after = json!({"code": 123});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "code".to_string(),
            Change::Modify {
                old: json!("123"),
                new: json!(123),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn mixed_changes_in_user_profile() {
        let before = json!({
            "user": {
                "name": "John",
                "age": 30,
                "settings": {"theme": "dark", "notifications": true}
            },
            "status": "active"
        });
        let after = json!({
            "user": {
                "name": "John",
                "age": 31,
                "settings": {"theme": "light", "notifications": true, "language": "en-US"}
            },
            "status": "inactive"
        });
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "user.age".to_string(),
            Change::Modify {
                old: json!(30),
                new: json!(31),
            },
        );
        expected.insert(
            "user.settings.theme".to_string(),
            Change::Modify {
                old: json!("dark"),
                new: json!("light"),
            },
        );
        expected.insert(
            "user.settings.language".to_string(),
            Change::Add(json!("en-US")),
        );
        expected.insert(
            "status".to_string(),
            Change::Modify {
                old: json!("active"),
                new: json!("inactive"),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn object_with_numeric_keys() {
        let before = json!({"0": "zero", "1": "one"});
        let after = json!({"0": "ZERO", "2": "two"});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "0".to_string(),
            Change::Modify {
                old: json!("zero"),
                new: json!("ZERO"),
            },
        );
        expected.insert("1".to_string(), Change::Remove(json!("one")));
        expected.insert("2".to_string(), Change::Add(json!("two")));
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn complex_nested_structure_with_item_lists() {
        let before = json!({
            "inventory": {"products": [
                {"id": 1, "name": "Product 1"},
                {"id": 2, "name": "Product 2"}
            ]}
        });
        let after = json!({
            "inventory": {"products": [
                {"id": 1, "name": "Updated Product 1"},
                {"id": 2, "name": "Product 2"}
            ]}
        });
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "inventory.products".to_string(),
            Change::Modify {
                old: json!([
                    {"id": 1, "name": "Product 1"},
                    {"id": 2, "name": "Product 2"}
                ]),
                new: json!([
                    {"id": 1, "name": "Updated Product 1"},
                    {"id": 2, "name": "Product 2"}
                ]),
            },
        );
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn extreme_empty_to_populated() {
        let before = json!({});
        let after = json!({"code": 1, "detail": {"value": 2}});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("code".to_string(), Change::Add(json!(1)));
        expected.insert("detail".to_string(), Change::Add(json!({"value": 2})));
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn extreme_populated_to_empty() {
        let before = json!({"code": 1, "detail": {"value": 2}});
        let after = json!({});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("code".to_string(), Change::Remove(json!(1)));
        expected.insert("detail".to_string(), Change::Remove(json!({"value": 2})));
        assert_eq!(delta, expected);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn applying_empty_delta_should_not_change_object() {
        let object = json!({"test": "value"});
        let empty_delta: Delta = Delta::new();
        let applied = apply(&object, &empty_delta);
        assert_eq!(applied, object);

        // apply should return a new object (clone), not a reference to the same one.
        assert!(!std::ptr::eq(&applied, &object));
    }

    #[test]
    fn simultaneous_multiple_changes_application() {
        let before = json!({"a": 1, "b": 2, "c": {"d": 3}});
        let mut delta: Delta = Delta::new();
        delta.insert(
            "a".to_string(),
            Change::Modify {
                old: json!(1),
                new: json!(10),
            },
        );
        delta.insert("b".to_string(), Change::Remove(json!(2)));
        delta.insert(
            "c.d".to_string(),
            Change::Modify {
                old: json!(3),
                new: json!(30),
            },
        );
        delta.insert("c.e".to_string(), Change::Add(json!(40)));
        delta.insert("f".to_string(), Change::Add(json!(50)));

        let expected = json!({"a": 10, "c": {"d": 30, "e": 40}, "f": 50});
        let applied = apply(&before, &delta);
        assert_eq!(applied, expected);
    }

    #[test]
    fn special_characters_in_keys_and_values() {
        let before = json!({
            "key with spaces": "value",
            "key-with-dashes": "test",
            "key_with_underscores": "data"
        });
        // NOTE: apply logic splits keys by '.', so "key.with.dots" becomes nested.
        let after = json!({
            "key with spaces": "new value",
            "key-with-dashes": "updated",
            "key.with.dots": "added"
        });
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "key with spaces".to_string(),
            Change::Modify {
                old: json!("value"),
                new: json!("new value"),
            },
        );
        expected.insert(
            "key-with-dashes".to_string(),
            Change::Modify {
                old: json!("test"),
                new: json!("updated"),
            },
        );
        expected.insert(
            "key_with_underscores".to_string(),
            Change::Remove(json!("data")),
        );
        expected.insert("key.with.dots".to_string(), Change::Add(json!("added")));
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        let expected_applied = json!({
            "key with spaces": "new value",
            "key-with-dashes": "updated",
            "key": {"with": {"dots": "added"}}
        });
        assert_eq!(applied, expected_applied);
    }

    #[test]
    fn unicode_and_emoji_handling() {
        let before = json!({"text": "olá mundo", "emoji": "🚀"});
        let after = json!({"text": "hello world", "emoji": "🎉", "new": "ñoño"});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "text".to_string(),
            Change::Modify {
                old: json!("olá mundo"),
                new: json!("hello world"),
            },
        );
        expected.insert(
            "emoji".to_string(),
            Change::Modify {
                old: json!("🚀"),
                new: json!("🎉"),
            },
        );
        expected.insert("new".to_string(), Change::Add(json!("ñoño")));
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn large_numbers_and_precision() {
        let before = json!({"big": 9223372036854775807i64, "float": PI});
        let after = json!({"big": -9223372036854775808i64, "float": E});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "big".to_string(),
            Change::Modify {
                old: json!(9223372036854775807i64),
                new: json!(-9223372036854775808i64),
            },
        );
        expected.insert(
            "float".to_string(),
            Change::Modify {
                old: json!(PI),
                new: json!(E),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn nested_arrays_with_objects() {
        let before = json!({"items": [{"id": 1}, {"id": 2}], "tags": ["a", "b"]});
        let after = json!({"items": [{"id": 1, "name": "item1"}], "tags": ["a", "b", "c"]});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "items".to_string(),
            Change::Modify {
                old: json!([{"id": 1}, {"id": 2}]),
                new: json!([{"id": 1, "name": "item1"}]),
            },
        );
        expected.insert(
            "tags".to_string(),
            Change::Modify {
                old: json!(["a", "b"]),
                new: json!(["a", "b", "c"]),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn empty_arrays_handling() {
        let before = json!({"empty": [], "filled": [1, 2, 3]});
        let after = json!({"empty": [1], "filled": []});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "empty".to_string(),
            Change::Modify {
                old: json!([]),
                new: json!([1]),
            },
        );
        expected.insert(
            "filled".to_string(),
            Change::Modify {
                old: json!([1, 2, 3]),
                new: json!([]),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn deeply_nested_object_operations() {
        let before = json!({"a": {"b": {"c": {"d": {"e": "deep_value"}}}}});
        let after = json!({"a": {"b": {"c": {"d": {"f": "new_deep_value"}}}}});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("a.b.c.d.e".to_string(), Change::Remove(json!("deep_value")));
        expected.insert(
            "a.b.c.d.f".to_string(),
            Change::Add(json!("new_deep_value")),
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn complete_object_replacement() {
        let before = json!({"config": {"theme": "dark", "lang": "en"}, "user": {"name": "John"}});
        let after =
            json!({"config": {"version": "2.0", "enabled": true}, "user": {"name": "John"}});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("config.theme".to_string(), Change::Remove(json!("dark")));
        expected.insert("config.lang".to_string(), Change::Remove(json!("en")));
        expected.insert("config.version".to_string(), Change::Add(json!("2.0")));
        expected.insert("config.enabled".to_string(), Change::Add(json!(true)));
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn mixed_types_in_arrays() {
        let before = json!({"mixed": [1, "string", true, null, {"nested": "object"}]});
        let after = json!({"mixed": [1, "string", false, {"nested": "updated"}, [1, 2, 3]]});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "mixed".to_string(),
            Change::Modify {
                old: json!([1, "string", true, null, {"nested": "object"}]),
                new: json!([1, "string", false, {"nested": "updated"}, [1, 2, 3]]),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn prune_empty_objects_after_removal() {
        let before = json!({"a": {"b": {"c": "value"}}, "d": "keep"});
        // delta sees whole "a" object as removed, doesn't recurse.
        let after = json!({"d": "keep"});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "a".to_string(),
            Change::Remove(json!({"b": {"c": "value"}})),
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn no_pruning_without_top_level_changes() {
        let before = json!({"a": {"b": {"c": "old"}}});
        let after = json!({"a": {"b": {"c": "new"}}});
        let delta: Delta = diff(&before, &after);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);

        assert!(
            applied
                .get("a")
                .unwrap()
                .get("b")
                .unwrap()
                .get("c")
                .is_some()
        );
    }

    #[test]
    fn complex_revert_operations() {
        let original =
            json!({"users": [{"id": 1, "name": "Alice"}], "settings": {"theme": "light"}});
        let modified = json!({
            "users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}],
            "settings": {"theme": "dark", "lang": "pt"}
        });
        let delta: Delta = diff(&original, &modified);
        let applied = apply(&original, &delta);
        assert_eq!(applied, modified);

        let reverted = revert(&modified, &delta);
        assert_eq!(reverted, original);
    }

    #[test]
    fn apply_with_invalid_paths() {
        let base = json!({"a": "value"});
        let mut delta: Delta = Delta::new();
        delta.insert("a.b.c".to_string(), Change::Add(json!("new_value")));

        let result = apply(&base, &delta);
        let expected = json!({"a": {"b": {"c": "new_value"}}});
        assert_eq!(result, expected);
    }

    #[test]
    fn large_nested_structure() {
        let mut before_map = serde_json::Map::new();
        let mut after_map = serde_json::Map::new();

        for i in 0..100 {
            before_map.insert(format!("key_{i}"), json!({"value": i}));
            after_map.insert(format!("key_{i}"), json!({"value": i + 100}));
        }

        let before = Value::Object(before_map);
        let after = Value::Object(after_map);
        let delta: Delta = diff(&before, &after);

        assert_eq!(delta.len(), 100);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn complex_cross_references() {
        let before = json!({
            "user1": {"friend": "user2", "data": {"score": 100}},
            "user2": {"friend": "user1", "data": {"score": 200}}
        });
        let after = json!({
            "user1": {"friend": "user3", "data": {"score": 150}},
            "user2": {"friend": "user1", "data": {"score": 200}},
            "user3": {"friend": "user1", "data": {"score": 50}}
        });

        let delta: Delta = diff(&before, &after);
        let applied = apply(&before, &delta);
        assert_eq!(applied, after);

        let reverted = revert(&after, &delta);
        assert_eq!(reverted, before);
    }

    #[test]
    fn json_special_characters() {
        let before = json!({"text": "line1\nline2\t\"quoted\""});
        let after = json!({"text": "line1\nline2\t\"updated\""});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "text".to_string(),
            Change::Modify {
                old: json!("line1\nline2\t\"quoted\""),
                new: json!("line1\nline2\t\"updated\""),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn empty_string_handling() {
        let before = json!({"empty": "", "filled": "content"});
        let after = json!({"empty": "now_filled", "filled": ""});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "empty".to_string(),
            Change::Modify {
                old: json!(""),
                new: json!("now_filled"),
            },
        );
        expected.insert(
            "filled".to_string(),
            Change::Modify {
                old: json!("content"),
                new: json!(""),
            },
        );
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn array_like_object_keys() {
        let before = json!({"0": "zero", "1": "one", "10": "ten"});
        let after = json!({"0": "ZERO", "2": "two", "10": "ten"});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert(
            "0".to_string(),
            Change::Modify {
                old: json!("zero"),
                new: json!("ZERO"),
            },
        );
        expected.insert("1".to_string(), Change::Remove(json!("one")));
        expected.insert("2".to_string(), Change::Add(json!("two")));
        assert_eq!(delta, expected);

        let applied = apply(&before, &delta);
        assert_eq!(applied, after);
    }

    #[test]
    fn multiple_delta_apply_cycles() {
        let mut current = json!({"counter": 0});

        for i in 1..=10 {
            let next = json!({"counter": i});
            let delta: Delta = diff(&current, &next);
            let applied = apply(&current, &delta);
            assert_eq!(applied, next);

            let reverted = revert(&next, &delta);
            assert_eq!(reverted, current);

            current = next;
        }
    }

    #[test]
    fn delta_consistency() {
        let a = json!({"x": 1, "y": {"z": 2}});
        let b = json!({"x": 10, "y": {"z": 20}, "w": 30});

        let delta_a_to_b: Delta = diff(&a, &b);
        let delta_b_to_a: Delta = diff(&b, &a);

        let b_from_a = apply(&a, &delta_a_to_b);
        assert_eq!(b_from_a, b);
        let a_from_b = apply(&b, &delta_b_to_a);
        assert_eq!(a_from_b, a);
    }

    #[test]
    fn extremely_deep_nesting() {
        let mut deep_before = json!("base");
        for i in (0..20).rev() {
            deep_before = json!({format!("level_{}", i): deep_before});
        }

        let mut deep_after = json!("modified");
        for i in (0..20).rev() {
            deep_after = json!({format!("level_{}", i): deep_after});
        }

        let delta: Delta = diff(&deep_before, &deep_after);
        let mut expected = Delta::new();
        expected.insert(
            "level_0.level_1.level_2.level_3.level_4.level_5.level_6.level_7.level_8.level_9.level_10.level_11.level_12.level_13.level_14.level_15.level_16.level_17.level_18.level_19".to_string(),
            Change::Modify { old: json!("base"), new: json!("modified") },
        );
        assert_eq!(delta, expected);

        let applied = apply(&deep_before, &delta);
        assert_eq!(applied, deep_after);
    }

    #[test]
    fn empty_to_populated() {
        let before = json!({});
        let after = json!({"code": 1, "detail": {"value": 2}});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("code".to_string(), Change::Add(json!(1)));
        expected.insert("detail".to_string(), Change::Add(json!({"value": 2})));

        assert_eq!(delta, expected);
        assert_eq!(revert(&after, &delta), before);
        assert_eq!(apply(&before, &delta), after);
    }

    #[test]
    fn populated_to_empty() {
        let before = json!({"code": 1, "detail": {"value": 2}});
        let after = json!({});
        let delta: Delta = diff(&before, &after);

        let mut expected = Delta::new();
        expected.insert("code".to_string(), Change::Remove(json!(1)));
        expected.insert("detail".to_string(), Change::Remove(json!({"value": 2})));

        assert_eq!(delta, expected);
        assert_eq!(revert(&after, &delta), before);
        assert_eq!(apply(&before, &delta), after);
    }
}
