// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use crate::error::{FMLError, Result};
use crate::frontend::DefaultBlock;
use crate::intermediate_representation::{CoproductDef, FeatureDef, ObjectDef, PropDef, TypeRef};

pub struct DefaultsMerger<'object> {
    objects: &'object BTreeMap<String, ObjectDef>,
    coproducts: &'object BTreeMap<String, CoproductDef>,

    supported_channels: Vec<String>,
    channel: Option<String>,
}

impl<'object> DefaultsMerger<'object> {
    pub fn new(
        objects: &'object BTreeMap<String, ObjectDef>,
        coproducts: &'object BTreeMap<String, CoproductDef>,
        supported_channels: Vec<String>,
        channel: Option<String>,
    ) -> Self {
        Self {
            objects,
            coproducts,
            supported_channels,
            channel,
        }
    }

    #[cfg(test)]
    pub fn new_with_channel(
        objects: &'object BTreeMap<String, ObjectDef>,
        coproducts: &'object BTreeMap<String, CoproductDef>,
        supported_channels: Vec<String>,
        channel: String,
    ) -> Self {
        Self::new(
            objects,
            coproducts,
            supported_channels,
            Some(channel.to_string()),
        )
    }

    fn collect_feature_defaults(&self, feature: &FeatureDef) -> serde_json::Value {
        self.collect_props_defaults(&feature.props)
    }

    fn get_object(&self, name: &str) -> &ObjectDef {
        self.objects
            .get(name)
            .unwrap_or_else(|| panic!("Object named {} is not defined", name))
    }

    fn get_coproduct(&self, name: &str) -> &CoproductDef {
        self.coproducts
            .get(name)
            .unwrap_or_else(|| panic!("Coproduct named {} is not defined", name))
    }

    fn collect_object_defaults(&self, name: &str) -> serde_json::Value {
        self.collect_props_defaults(&self.get_object(name).props)
    }

    fn collect_props_defaults(&self, props: &Vec<PropDef>) -> Value {
        let mut res = serde_json::value::Map::new();
        for p in props {
            res.insert(p.name(), self.collect_prop_defaults(&p.typ, &p.default));
        }
        serde_json::Value::Object(res)
    }

    fn collect_prop_defaults(&self, typ: &TypeRef, v: &serde_json::Value) -> serde_json::Value {
        match typ {
            TypeRef::Object(name) => self.merge_values(typ, &self.collect_object_defaults(name), v),
            TypeRef::Coproduct(name) => {
                let coproduct = self.get_coproduct(name);
                match v.as_object().and_then(|map| coproduct.tag(map)) {
                    Some(tag) if coproduct.has_variant(tag) => self.merge_values(
                        &TypeRef::Object(tag.to_string()),
                        &self.collect_object_defaults(tag),
                        v,
                    ),
                    // The tag is not valid: the DefaultsValidator reports this.
                    _ => v.clone(),
                }
            }
            TypeRef::EnumMap(_, v_type) => self.collect_map_defaults(v_type, v),
            TypeRef::StringMap(v_type) => self.collect_map_defaults(v_type, v),
            _ => v.clone(),
        }
    }

    fn collect_map_defaults(&self, v_type: &TypeRef, obj: &serde_json::Value) -> serde_json::Value {
        // A default that is not a JSON object, for example a missing default, is not valid for a
        // map. The DefaultsValidator reports this.
        let Some(map) = obj.as_object() else {
            return obj.clone();
        };
        let mut res = serde_json::value::Map::new();
        for (k, v) in map {
            let collected = self.collect_prop_defaults(v_type, v);
            res.insert(k.clone(), collected);
        }
        serde_json::Value::Object(res)
    }

    /// Transforms a feature definition with unmerged defaults into a feature
    /// definition with its defaults merged.
    ///
    /// # How the algorithm works:
    /// There are two types of defaults:
    /// 1. Field level defaults
    /// 1. Feature level defaults, that are listed by channel
    ///
    /// The algorithm gathers the field level defaults first, they are the base
    /// defaults. Then, it gathers the feature level defaults and merges them by
    /// calling [`collect_channel_defaults`]. Finally, it overwrites any common
    /// defaults between the merged feature level defaults and the field level defaults
    ///
    /// # Example:
    /// Assume we have the following feature manifest
    /// ```yaml
    ///  variables:
    ///   positive:
    ///   description: This is a positive button
    ///   type: Button
    ///   default:
    ///     {
    ///       "label": "Ok then",
    ///       "color": "blue"
    ///     }
    ///  default:
    ///      - channel: release
    ///      value: {
    ///        "positive": {
    ///          "color": "green"
    ///        }
    ///      }
    ///      - value: {
    ///      "positive": {
    ///        "alt-text": "Go Ahead!"
    ///      }
    /// }
    /// ```
    ///
    /// The result of the algorithm would be a default that looks like:
    /// ```yaml
    /// variables:
    ///     positive:
    ///     default:
    ///     {
    ///         "label": "Ok then",
    ///         "color": "green",
    ///         "alt-text": "Go Ahead!"
    ///     }
    ///
    /// ```
    ///
    /// - The `label` comes from the original field level default
    /// - The `color` comes from the `release` channel feature level default
    /// - The `alt-text` comes from the feature level default with no channel (that applies to all channels)
    ///
    /// # Arguments
    /// - `feature_def`: a [`FeatureDef`] representing the feature definition to transform
    /// - `channel`: a [`Option<&String>`] representing the channel to merge back into the field variables.
    ///   If the `channel` is `None` we default to using the `release` channel.
    /// - `supported_channels`: a [`&[String]`] representing the channels that are supported by the manifest
    ///
    /// # Returns
    /// Returns a transformed [`FeatureDef`] with its defaults merged
    pub fn merge_feature_defaults(
        &self,
        feature_def: &mut FeatureDef,
        defaults: &Option<Vec<DefaultBlock>>,
    ) -> Result<(), FMLError> {
        let variable_defaults = self.collect_feature_defaults(feature_def);
        let defaults_to_merge = self.channel_specific_defaults(&feature_def.props, defaults)?;
        let merged =
            self.merge_props_values(&feature_def.props, &variable_defaults, &defaults_to_merge);

        self.overwrite_defaults(feature_def, &merged);
        Ok(())
    }

    /// Mutates a FeatureDef by changing the defaults to the `merged` value.
    ///
    /// This does not do any _merging_ of defaults with the passed value:
    /// you can get the merged value from `merge_feature_config`.
    ///
    /// It also does not attempt to validate the keys against the property:
    /// this is done in the `get_errors` of `DefaultsValidator`, more specifically, the
    /// `validate_props_types` method.
    ///
    /// The `merged` value is expected to be a JSON object.
    pub(crate) fn overwrite_defaults(&self, feature_def: &mut FeatureDef, merged: &Value) {
        let map = merged.as_object().expect("`merged` value not a map");

        for p in &mut feature_def.props {
            if let Some(v) = map.get(&p.name) {
                p.default = v.clone();
            }
        }
    }

    fn channel_specific_defaults(
        &self,
        props: &[PropDef],
        defaults: &Option<Vec<DefaultBlock>>,
    ) -> Result<Value> {
        let supported_channels = self.supported_channels.as_slice();
        let channel = &self.channel;
        if let Some(channel) = channel
            && !supported_channels.iter().any(|c| c == channel)
        {
            return Err(FMLError::InvalidChannelError(
                channel.into(),
                supported_channels.into(),
            ));
        }
        let empty_object = json!({});
        if let Some(defaults) = defaults {
            // No channel is represented by an unlikely string.
            let no_channel = "NO CHANNEL SPECIFIED".to_string();
            let merge = |old: &Value, new: &Value| self.merge_props_values(props, old, new);
            let merged_defaults =
                collect_channel_defaults_with(defaults, supported_channels, &no_channel, &merge)?;
            let channel = self.channel.as_ref().unwrap_or(&no_channel);
            let merged = merged_defaults[channel].clone();
            Ok(merged)
        } else {
            Ok(empty_object)
        }
    }

    /// A convenience method to get the defaults from the feature, and merger it
    /// with the passed value.
    pub(crate) fn merge_feature_config(&self, feature_def: &FeatureDef, value: &Value) -> Value {
        let defaults = self.collect_feature_defaults(feature_def);
        self.merge_props_values(&feature_def.props, &defaults, value)
    }

    /// Merges `new` on top of `old`, where both are JSON objects with the given `props`.
    ///
    /// Keys that are not in `props` are merged with [`merge_two_defaults`]: the
    /// DefaultsValidator reports them.
    fn merge_props_values(&self, props: &[PropDef], old: &Value, new: &Value) -> Value {
        let (Value::Object(old_map), Value::Object(new_map)) = (old, new) else {
            return new.clone();
        };
        let mut merged = old_map.clone();
        for (key, new_value) in new_map {
            let value = match (props.iter().find(|p| &p.name == key), old_map.get(key)) {
                (Some(prop), Some(old_value)) => self.merge_values(&prop.typ, old_value, new_value),
                (None, Some(old_value)) => merge_two_defaults(old_value, new_value),
                (_, None) => new_value.clone(),
            };
            merged.insert(key.clone(), value);
        }
        Value::Object(merged)
    }

    /// Merges `new` on top of `old`, where both are values of the type `typ`.
    ///
    /// For all types except coproducts, this gives the same result as [`merge_two_defaults`].
    ///
    /// For a coproduct, if `old` and `new` have the same tag, then the two values are merged as
    /// objects of that variant. If the tags are different, then the result starts from the
    /// defaults of the new variant's object, and `old` is discarded.
    fn merge_values(&self, typ: &TypeRef, old: &Value, new: &Value) -> Value {
        match (typ, old, new) {
            (TypeRef::Object(name), Value::Object(_), Value::Object(_)) => {
                self.merge_props_values(&self.get_object(name).props, old, new)
            }
            (TypeRef::Coproduct(name), Value::Object(old_map), Value::Object(new_map)) => {
                let coproduct = self.get_coproduct(name);
                match (coproduct.tag(old_map), coproduct.tag(new_map)) {
                    (Some(old_tag), Some(new_tag))
                        if old_tag == new_tag && coproduct.has_variant(new_tag) =>
                    {
                        self.merge_props_values(&self.get_object(new_tag).props, old, new)
                    }
                    (_, Some(new_tag)) if coproduct.has_variant(new_tag) => {
                        let object = self.get_object(new_tag);
                        let base = self.collect_props_defaults(&object.props);
                        self.merge_props_values(&object.props, &base, new)
                    }
                    // The tag is not valid: the DefaultsValidator reports this.
                    _ => new.clone(),
                }
            }
            (
                TypeRef::StringMap(value_type) | TypeRef::EnumMap(_, value_type),
                Value::Object(old_map),
                Value::Object(new_map),
            ) => {
                let mut merged = old_map.clone();
                for (key, new_value) in new_map {
                    let value = match old_map.get(key) {
                        Some(old_value) => self.merge_values(value_type, old_value, new_value),
                        None => new_value.clone(),
                    };
                    merged.insert(key.clone(), value);
                }
                Value::Object(merged)
            }
            (TypeRef::Option(inner), _, _) => self.merge_values(inner, old, new),
            // Values that are not objects are not merged: the new value replaces the old one.
            // Objects with an unexpected type are merged as before: the DefaultsValidator reports them.
            _ => merge_two_defaults(old, new),
        }
    }
}

/// Merges two [`serde_json::Value`]s into one
///
/// # Arguments:
/// - `old_default`: a reference to a [`serde_json::Value`], that represents the old default
/// - `new_default`: a reference to a [`serde_json::Value`], that represents the new default, this takes
///   precedence over the `old_default` if they have conflicting fields
///
/// # Returns
/// A merged [`serde_json::Value`] that contains all fields from `old_default` and `new_default`, merging
/// where there is a conflict. If the `old_default` and `new_default` are not both objects, this function
/// returns the `new_default`
fn merge_two_defaults(
    old_default: &serde_json::Value,
    new_default: &serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value::Object;
    match (old_default.clone(), new_default.clone()) {
        (Object(old), Object(new)) => {
            let mut merged = serde_json::Map::new();
            for (key, val) in old {
                merged.insert(key, val);
            }
            for (key, val) in new {
                if let Some(old_val) = merged.get(&key).cloned() {
                    merged.insert(key, merge_two_defaults(&old_val, &val));
                } else {
                    merged.insert(key, val);
                }
            }
            Object(merged)
        }
        (_, new) => new,
    }
}

/// Collects the channel defaults of the feature manifest
/// and merges them by channel
///
/// **NOTE**: defaults with no channel apply to **all** channels
///
/// # Arguments
/// - `defaults`: a [`serde_json::Value`] representing the array of defaults
///
/// # Returns
/// Returns a [`std::collections::HashMap<String, serde_json::Value>`] representing
/// the merged defaults. The key is the name of the channel and the value is the
/// merged json.
///
/// # Errors
/// Will return errors in the following cases (not exhaustive):
/// - The `defaults` argument is not an array
/// - There is a `channel` in the `defaults` argument that doesn't
///   exist in the `channels` argument
#[cfg(test)]
fn collect_channel_defaults(
    defaults: &[DefaultBlock],
    channels: &[String],
    no_channel: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    collect_channel_defaults_with(defaults, channels, no_channel, &merge_two_defaults)
}

/// Like [`collect_channel_defaults`], but with a function to merge one default on top of another.
fn collect_channel_defaults_with(
    defaults: &[DefaultBlock],
    channels: &[String],
    no_channel: &str,
    merge: &dyn Fn(&Value, &Value) -> Value,
) -> Result<HashMap<String, serde_json::Value>> {
    // We initialize the map to have an entry for every valid channel
    let mut channel_map = channels
        .iter()
        .map(|channel_name| (channel_name.clone(), json!({})))
        .collect::<HashMap<_, _>>();
    channel_map.insert(no_channel.to_string(), json!({}));
    for default in defaults {
        if let Some(channels_for_default) = &default.merge_channels() {
            for channel in channels_for_default {
                if let Some(old_default) = channel_map.get(channel).cloned() {
                    if default.targeting.is_none() {
                        // TODO: we currently ignore any defaults with targeting involved
                        let merged = merge(&old_default, &default.value);
                        channel_map.insert(channel.clone(), merged);
                    }
                } else {
                    return Err(FMLError::InvalidChannelError(
                        channel.into(),
                        channels.into(),
                    ));
                }
            }
        // This is a default with no channel, so it applies to all channels
        } else {
            channel_map = channel_map
                .into_iter()
                .map(|(channel, old_default)| (channel, merge(&old_default, &default.value)))
                .collect();
        }
    }
    Ok(channel_map)
}

#[cfg(test)]
mod unit_tests {
    use crate::intermediate_representation::PropDef;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_merge_two_defaults_both_objects_no_intersection() -> Result<()> {
        let old_default = json!({
            "button-color": "blue",
            "dialog_option": "greetings",
            "is_enabled": false,
            "num_items": 5
        });
        let new_default = json!({
            "new_homepage": true,
            "item_order": ["first", "second", "third"],
        });
        let merged = merge_two_defaults(&old_default, &new_default);
        assert_eq!(
            json!({
                "button-color": "blue",
                "dialog_option": "greetings",
                "is_enabled": false,
                "num_items": 5,
                "new_homepage": true,
                "item_order": ["first", "second", "third"],
            }),
            merged
        );
        Ok(())
    }

    #[test]
    fn test_merge_two_defaults_intersecting_different_types() -> Result<()> {
        // if there is an intersection, but they are different types, we just take the new one
        let old_default = json!({
            "button-color": "blue",
            "dialog_option": "greetings",
            "is_enabled": {
                "value": false
            },
            "num_items": 5
        });
        let new_default = json!({
            "new_homepage": true,
            "is_enabled": true,
            "item_order": ["first", "second", "third"],
        });
        let merged = merge_two_defaults(&old_default, &new_default);
        assert_eq!(
            json!({
                "button-color": "blue",
                "dialog_option": "greetings",
                "is_enabled": true,
                "num_items": 5,
                "new_homepage": true,
                "item_order": ["first", "second", "third"],
            }),
            merged
        );
        Ok(())
    }

    #[test]
    fn test_merge_two_defaults_non_map_intersection() -> Result<()> {
        // if they intersect on both key and type, but the type intersected is not an object, we just take the new one
        let old_default = json!({
            "button-color": "blue",
            "dialog_option": "greetings",
            "is_enabled": false,
            "num_items": 5
        });
        let new_default = json!({
            "button-color": "green",
            "new_homepage": true,
            "is_enabled": true,
            "num_items": 10,
            "item_order": ["first", "second", "third"],
        });
        let merged = merge_two_defaults(&old_default, &new_default);
        assert_eq!(
            json!({
                "button-color": "green",
                "dialog_option": "greetings",
                "is_enabled": true,
                "num_items": 10,
                "new_homepage": true,
                "item_order": ["first", "second", "third"],
            }),
            merged
        );
        Ok(())
    }

    #[test]
    fn test_merge_two_defaults_map_intersection_recursive_merge() -> Result<()> {
        // if they intersect on both key and type, but the type intersected is not an object, we just take the new one
        let old_default = json!({
            "button-color": "blue",
            "dialog_item": {
                "title": "hello",
                "message": "bobo",
                "priority": 10,
            },
            "is_enabled": false,
            "num_items": 5
        });
        let new_default = json!({
            "button-color": "green",
            "new_homepage": true,
            "is_enabled": true,
            "dialog_item": {
                "message": "fofo",
                "priority": 11,
                "subtitle": "hey there"
            },
            "num_items": 10,
            "item_order": ["first", "second", "third"],
        });
        let merged = merge_two_defaults(&old_default, &new_default);
        assert_eq!(
            json!({
                "button-color": "green",
                "dialog_item": {
                    "title": "hello",
                    "message": "fofo",
                    "priority": 11,
                    "subtitle": "hey there"
                },
                "is_enabled": true,
                "num_items": 10,
                "new_homepage": true,
                "item_order": ["first", "second", "third"],
            }),
            merged
        );
        Ok(())
    }

    #[test]
    fn test_merge_two_defaults_highlevel_non_maps() -> Result<()> {
        let old_default = json!(["array", "json"]);
        let new_default = json!(["another", "array"]);
        let merged = merge_two_defaults(&old_default, &new_default);
        assert_eq!(json!(["another", "array"]), merged);
        Ok(())
    }

    #[test]
    fn test_channel_defaults_channels_no_merging() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "dark-green"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "light-green"
                    })
                ),
                ("".to_string(), json!({}),),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_channels_merging_same_channel() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green",
                    "title": "heya"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-red",
                    "subtitle": "hello",
                }
            },
            {
                "channel": "beta",
                "value": {
                    "title": "hello there"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "dark-red",
                        "title": "heya",
                        "subtitle": "hello"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "light-green",
                        "title": "hello there"
                    })
                ),
                ("".to_string(), json!({}),),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_no_channel_applies_to_all() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
            {
                "value": {
                    "title": "heya"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green",
                        "title": "heya"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "dark-green",
                        "title": "heya"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "light-green",
                        "title": "heya"
                    })
                ),
                (
                    "".to_string(),
                    json!({
                        "title": "heya",
                    }),
                )
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_no_channel_overwrites_all() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
            {
                "value": {
                    "button-color": "red"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "red"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "red"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "red"
                    })
                ),
                (
                    "".to_string(),
                    json!({
                        "button-color": "red",
                    }),
                )
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_no_channel_gets_overwritten_if_followed_by_channel() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
            {
                "value": {
                    "button-color": "red"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-red"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "red"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "dark-red"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "red"
                    })
                ),
                (
                    "".to_string(),
                    json!({
                        "button-color": "red",
                    }),
                )
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_channels_multiple() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channels": ["release", "beta"],
                "value": {
                    "button-color": "green"
                }
            },
        ]))?;
        let res =
            collect_channel_defaults(&input, &["release".to_string(), "beta".to_string()], "")?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                ("".to_string(), json!({}),)
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_channel_multiple_merge_channels_multiple() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "nightly, debug",
                "channels": ["release", "beta"],
                "value": {
                    "button-color": "green"
                }
            },
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "beta".to_string(),
                "nightly".to_string(),
                "debug".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "beta".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "debug".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                ("".to_string(), json!({}),)
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_channel_defaults_fail_if_invalid_channel_supplied() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
            {
                "channel": "bobo",
                "value": {
                    "button-color": "no color"
                }
            }
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )
        .expect_err("Should return error");
        if let FMLError::InvalidChannelError(channel, _supported) = res {
            assert!(channel.contains("bobo"));
        } else {
            panic!(
                "Should have returned a InvalidChannelError, returned {:?}",
                res
            )
        }
        Ok(())
    }

    #[test]
    fn test_channel_defaults_empty_default_created_if_none_supplied_in_feature() -> Result<()> {
        let input: Vec<DefaultBlock> = serde_json::from_value(json!([
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            // No entry fo beta supplied, we will still get an entry in the result
            // but it will be empty
        ]))?;
        let res = collect_channel_defaults(
            &input,
            &[
                "release".to_string(),
                "nightly".to_string(),
                "beta".to_string(),
            ],
            "",
        )?;
        assert_eq!(
            vec![
                (
                    "release".to_string(),
                    json!({
                        "button-color": "green"
                    })
                ),
                (
                    "nightly".to_string(),
                    json!({
                        "button-color": "dark-green"
                    })
                ),
                ("beta".to_string(), json!({})),
                ("".to_string(), json!({}),)
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            res
        );
        Ok(())
    }

    #[test]
    fn test_merge_feature_default_unsupported_channel() -> Result<()> {
        let mut feature_def: FeatureDef = Default::default();
        let objects = Default::default();
        let coproducts = Default::default();
        let merger = DefaultsMerger::new_with_channel(
            &objects,
            &coproducts,
            vec!["release".into(), "beta".into()],
            "nightly".into(),
        );
        let err = merger
            .merge_feature_defaults(&mut feature_def, &None)
            .expect_err("Should return an error");
        if let FMLError::InvalidChannelError(channel, _supported) = err {
            assert!(channel.contains("nightly"));
        } else {
            panic!(
                "Should have returned an InvalidChannelError, returned: {:?}",
                err
            );
        }
        Ok(())
    }

    #[test]
    fn test_merge_feature_default_overwrite_field_default_based_on_channel() -> Result<()> {
        let mut feature_def = FeatureDef {
            props: vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("blue"),
            )],
            ..Default::default()
        };
        let default_blocks = serde_json::from_value(json!([
            {
                "channel": "nightly",
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
        ]))?;
        let objects = Default::default();
        let coproducts = Default::default();
        let merger = DefaultsMerger::new_with_channel(
            &objects,
            &coproducts,
            vec!["release".into(), "beta".into(), "nightly".into()],
            "nightly".into(),
        );
        merger.merge_feature_defaults(&mut feature_def, &default_blocks)?;
        assert_eq!(
            feature_def.props,
            vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("dark-green"),
            )]
        );
        Ok(())
    }

    #[test]
    fn test_merge_feature_default_field_default_not_overwritten_if_no_feature_default_for_channel()
    -> Result<()> {
        let mut feature_def = FeatureDef {
            props: vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("blue"),
            )],
            ..Default::default()
        };
        let default_blocks = serde_json::from_value(json!([{
            "channel": "release",
            "value": {
                "button-color": "green"
            }
        },
        {
            "channel": "beta",
            "value": {
                "button-color": "light-green"
            }
        }]))?;
        let objects = Default::default();
        let coproducts = Default::default();
        let merger = DefaultsMerger::new_with_channel(
            &objects,
            &coproducts,
            vec!["release".into(), "beta".into(), "nightly".into()],
            "nightly".into(),
        );
        merger.merge_feature_defaults(&mut feature_def, &default_blocks)?;
        assert_eq!(
            feature_def.props,
            vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("blue"),
            )]
        );
        Ok(())
    }

    #[test]
    fn test_merge_feature_default_overwrite_nested_field_default() -> Result<()> {
        let mut feature_def = FeatureDef {
            props: vec![PropDef::new(
                "Dialog",
                &TypeRef::String,
                &json!({
                    "button-color": "blue",
                    "title": "hello",
                    "inner": {
                        "bobo": "fofo",
                        "other-field": "other-value"
                    }
                }),
            )],

            ..Default::default()
        };
        let default_blocks = serde_json::from_value(json!([
            {
                "channel": "nightly",
                "value": {
                    "Dialog": {
                        "button-color": "dark-green",
                        "inner": {
                            "bobo": "nightly"
                        }
                    }
                }
            },
            {
                "channel": "release",
                "value": {
                    "Dialog": {
                        "button-color": "green",
                        "inner": {
                            "bobo": "release",
                            "new-field": "new-value"
                        }
                    }
                }
            },
            {
                "channel": "beta",
                "value": {
                    "Dialog": {
                        "button-color": "light-green",
                        "inner": {
                            "bobo": "beta"
                        }
                    }
                }
            },
        ]))?;
        let objects = Default::default();
        let coproducts = Default::default();
        let merger = DefaultsMerger::new_with_channel(
            &objects,
            &coproducts,
            vec!["release".into(), "beta".into(), "nightly".into()],
            "release".into(),
        );
        merger.merge_feature_defaults(&mut feature_def, &default_blocks)?;
        assert_eq!(
            feature_def.props,
            vec![PropDef::new(
                "Dialog",
                &TypeRef::String,
                &json!({
                        "button-color": "green",
                        "title": "hello",
                        "inner": {
                            "bobo": "release",
                            "other-field": "other-value",
                            "new-field": "new-value"
                        }
                })
            )]
        );
        Ok(())
    }

    #[test]
    fn test_merge_feature_default_overwrite_field_default_based_on_channel_using_only_no_channel_default()
    -> Result<()> {
        let mut feature_def = FeatureDef {
            props: vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("blue"),
            )],
            ..Default::default()
        };
        let default_blocks = serde_json::from_value(json!([
            // No channel applies to all channel
            // so the nightly channel will get this
            {
                "value": {
                    "button-color": "dark-green"
                }
            },
            {
                "channel": "release",
                "value": {
                    "button-color": "green"
                }
            },
            {
                "channel": "beta",
                "value": {
                    "button-color": "light-green"
                }
            },
        ]))?;
        let objects = Default::default();
        let coproducts = Default::default();
        let merger = DefaultsMerger::new_with_channel(
            &objects,
            &coproducts,
            vec!["release".into(), "beta".into(), "nightly".into()],
            "nightly".into(),
        );
        merger.merge_feature_defaults(&mut feature_def, &default_blocks)?;
        assert_eq!(
            feature_def.props,
            vec![PropDef::new(
                "button-color",
                &TypeRef::String,
                &json!("dark-green"),
            )]
        );
        Ok(())
    }
}
