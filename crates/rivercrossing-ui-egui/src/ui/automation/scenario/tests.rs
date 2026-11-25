// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the v1 scenario model, the `Expect` evaluator and the parity schema.

use serde_json::{Map, Value, json};

use super::{Node, Observation, Scenario, Step, WireTree};
use crate::ui::automation::selector::Selector;

/// The parity schema, read from the repository's `tools/parity` directory at compile time.
const SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/parity/schema.json"
));

/// One scenario that carries every step kind.
fn sample_scenario() -> Scenario {
    Scenario {
        version: 1,
        id: "smoke".to_owned(),
        size: Some([1100, 780]),
        steps: vec![
            Step::Command {
                action: "mi_start_ride".to_owned(),
                params: json!({"x": 1}),
            },
            Step::Click {
                selector: Selector::Action {
                    action: "mi_start_ride".to_owned(),
                },
            },
            Step::Type {
                selector: Selector::RoleAndLabel {
                    role: "TextInput".to_owned(),
                    label: "Plate".to_owned(),
                    window: None,
                },
                text: "abc".to_owned(),
            },
            Step::Key {
                key: "Enter".to_owned(),
                modifiers: vec!["ctrl".to_owned()],
            },
            Step::Snapshot {
                name: "shell".to_owned(),
            },
            Step::Expect {
                selector: Selector::Action {
                    action: "mi_start_ride".to_owned(),
                },
                name: Some("Start Ride".to_owned()),
                value: Some("clicked 1".to_owned()),
            },
        ],
    }
}

/// One observation over a two-node tree.
fn sample_observation() -> Observation {
    Observation {
        app: "rivercrossing".to_owned(),
        version: 1,
        viewport: [1100, 780],
        tree: WireTree {
            nodes: vec![
                Node {
                    id: 0,
                    role: "Window".to_owned(),
                    name: None,
                    value: None,
                    children: vec![2],
                },
                Node {
                    id: 2,
                    role: "Button".to_owned(),
                    name: Some("Start Ride".to_owned()),
                    value: None,
                    children: Vec::new(),
                },
            ],
            focus: Some(2),
            root: Some(0),
        },
        screenshots: vec!["console_empty.png".to_owned()],
    }
}

/// The flat wire tree the `Expect` tests check.
fn wire_tree() -> WireTree {
    WireTree {
        nodes: vec![
            Node {
                id: 0,
                role: "Window".to_owned(),
                name: None,
                value: None,
                children: vec![2, 3, 4],
            },
            Node {
                id: 2,
                role: "Button".to_owned(),
                name: Some("Start Ride".to_owned()),
                value: None,
                children: Vec::new(),
            },
            Node {
                id: 3,
                role: "TextInput".to_owned(),
                name: Some("Plate".to_owned()),
                value: Some("abc".to_owned()),
                children: Vec::new(),
            },
            Node {
                id: 4,
                role: "Label".to_owned(),
                name: None,
                value: Some("clicked 1".to_owned()),
                children: Vec::new(),
            },
        ],
        focus: None,
        root: Some(0),
    }
}

#[test]
fn test_scenario_round_trips_v1() {
    // Arrange
    let scenario = sample_scenario();

    // Act
    let value = serde_json::to_value(&scenario).expect("the scenario serializes");
    let back: Scenario = serde_json::from_value(value.clone()).expect("the scenario parses");

    // Assert
    assert_eq!(value.get("version"), Some(&json!(1)));
    assert_eq!(back, scenario);
}

#[test]
fn test_observation_round_trips_v1() {
    // Arrange
    let observation = sample_observation();

    // Act
    let value = serde_json::to_value(&observation).expect("the observation serializes");
    let back: Observation = serde_json::from_value(value.clone()).expect("the observation parses");

    // Assert
    assert_eq!(value.get("version"), Some(&json!(1)));
    assert_eq!(back, observation);
}

#[test]
fn test_step_key_serializes_its_shape() {
    // Arrange
    let step = Step::Key {
        key: "Enter".to_owned(),
        modifiers: vec!["ctrl".to_owned()],
    };

    // Act
    let value = serde_json::to_value(&step).expect("the key step serializes");

    // Assert
    assert_eq!(
        value,
        json!({"kind": "key", "key": "Enter", "modifiers": ["ctrl"]})
    );
}

#[test]
fn test_step_snapshot_serializes_its_shape() {
    // Arrange
    let step = Step::Snapshot {
        name: "shell".to_owned(),
    };

    // Act
    let value = serde_json::to_value(&step).expect("the snapshot step serializes");

    // Assert
    assert_eq!(value, json!({"kind": "snapshot", "name": "shell"}));
}

#[test]
fn test_step_command_defaults_its_params() {
    // Arrange
    let value = json!({"kind": "command", "action": "mi_start_ride"});

    // Act
    let step: Step = serde_json::from_value(value).expect("the command step parses");

    // Assert
    assert_eq!(
        step,
        Step::Command {
            action: "mi_start_ride".to_owned(),
            params: Value::Null,
        }
    );
}

#[test]
fn test_step_rejects_an_unknown_kind() {
    // Arrange
    let value = json!({"kind": "teleport"});

    // Act
    let parsed = serde_json::from_value::<Step>(value);

    // Assert
    let error = parsed.expect_err("an unknown step kind is rejected");
    assert!(
        error.to_string().contains("unknown variant `teleport`"),
        "the error names the kind: {error}"
    );
}

#[test]
fn test_observation_ignores_text_selection() {
    // Arrange
    let value = json!({
        "app": "rivercrossing",
        "version": 1,
        "viewport": [800, 600],
        "tree": {
            "nodes": [{
                "id": 2,
                "role": "TextInput",
                "name": "Plate",
                "children": [],
                "textSelection": {"anchor": 0, "focus": 1}
            }],
            "focus": null,
            "root": null
        },
        "screenshots": []
    });

    // Act
    let observation: Observation = serde_json::from_value(value).expect("the observation parses");

    // Assert
    assert_eq!(observation.version, 1);
    assert_eq!(
        observation
            .tree
            .nodes
            .first()
            .and_then(|node| node.name.as_deref()),
        Some("Plate")
    );
}

#[test]
fn test_expect_matches_the_named_node() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::RoleAndLabel {
            role: "textinput".to_owned(),
            label: "Plate".to_owned(),
            window: None,
        },
        name: Some("Plate".to_owned()),
        value: Some("abc".to_owned()),
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(checked, Ok(()));
}

#[test]
fn test_expect_matches_an_action_selector() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::Action {
            action: "mi_start_ride".to_owned(),
        },
        name: Some("Start Ride".to_owned()),
        value: None,
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(checked, Ok(()));
}

#[test]
fn test_expect_reports_a_name_mismatch() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::Action {
            action: "mi_start_ride".to_owned(),
        },
        name: Some("Plate".to_owned()),
        value: None,
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("node 2 name is \"Start Ride\", expected \"Plate\"".to_owned())
    );
}

#[test]
fn test_expect_reports_a_value_mismatch() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::RoleAndLabel {
            role: "TextInput".to_owned(),
            label: "Plate".to_owned(),
            window: None,
        },
        name: None,
        value: Some("xyz".to_owned()),
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("node 3 value is \"abc\", expected \"xyz\"".to_owned())
    );
}

#[test]
fn test_expect_reports_a_missing_value() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::Action {
            action: "mi_start_ride".to_owned(),
        },
        name: None,
        value: Some("clicked 1".to_owned()),
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("node 2 value is none, expected \"clicked 1\"".to_owned())
    );
}

#[test]
fn test_expect_reports_a_missing_node() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::RoleAndLabel {
            role: "Button".to_owned(),
            label: "Plate".to_owned(),
            window: None,
        },
        name: None,
        value: None,
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("no node matches role `Button` and label `Plate`".to_owned())
    );
}

#[test]
fn test_expect_reports_a_duplicate_match() {
    // Arrange
    let mut tree = wire_tree();
    tree.nodes.push(Node {
        id: 5,
        role: "Button".to_owned(),
        name: Some("Start Ride".to_owned()),
        value: None,
        children: Vec::new(),
    });
    let step = Step::Expect {
        selector: Selector::Action {
            action: "mi_start_ride".to_owned(),
        },
        name: None,
        value: None,
    };

    // Act
    let checked = step.evaluate(&tree);

    // Assert
    assert_eq!(
        checked,
        Err(
            "2 nodes match role `Button` and label `Start Ride`; a window scope is required"
                .to_owned()
        )
    );
}

#[test]
fn test_expect_reports_a_window_scope() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::RoleAndLabel {
            role: "Button".to_owned(),
            label: "Start Ride".to_owned(),
            window: Some("main".to_owned()),
        },
        name: None,
        value: None,
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("window-scoped selectors are not supported at EPIC-10 (window `main`)".to_owned())
    );
}

#[test]
fn test_expect_reports_an_unknown_action() {
    // Arrange
    let step = Step::Expect {
        selector: Selector::Action {
            action: "nope".to_owned(),
        },
        name: None,
        value: None,
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(checked, Err("unknown action `nope`".to_owned()));
}

#[test]
fn test_expect_rejects_another_step() {
    // Arrange
    let step = Step::Snapshot {
        name: "shell".to_owned(),
    };

    // Act
    let checked = step.evaluate(&wire_tree());

    // Assert
    assert_eq!(
        checked,
        Err("only an `expect` step can be evaluated".to_owned())
    );
}

#[test]
fn test_schema_matches_the_serde_derive() {
    // Arrange
    let schema: Value = serde_json::from_str(SCHEMA).expect("the parity schema parses");
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .expect("the schema has `$defs`");
    let mut names: Vec<&str> = defs.keys().map(String::as_str).collect();
    names.sort_unstable();

    // Act
    let scenario = serde_json::to_value(sample_scenario()).expect("the scenario serializes");
    let observation =
        serde_json::to_value(sample_observation()).expect("the observation serializes");

    // Assert
    assert_eq!(
        schema.get("$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema")
    );
    assert_eq!(
        names,
        [
            "Node",
            "Observation",
            "Scenario",
            "Selector",
            "Step",
            "WireTree"
        ]
    );
    assert_eq!(required(defs, "Scenario"), ["id", "steps", "version"]);
    assert_eq!(
        step_kinds(defs),
        ["click", "command", "expect", "key", "snapshot", "type"]
    );
    assert_documented(defs, "Scenario", &scenario);
    assert_documented(defs, "Observation", &observation);
    assert_documented(
        defs,
        "WireTree",
        observation.get("tree").expect("the observation has a tree"),
    );
    assert_documented(defs, "Node", first_node(&observation));
    assert_steps_documented(defs, &scenario);
}

/// The first node of `observation`'s wire tree.
fn first_node(observation: &Value) -> &Value {
    observation
        .get("tree")
        .and_then(|tree| tree.get("nodes"))
        .and_then(Value::as_array)
        .and_then(|nodes| nodes.first())
        .expect("the observation has one node")
}

/// The `required` keys of `def`, sorted.
fn required(defs: &Map<String, Value>, def: &str) -> Vec<String> {
    let mut keys: Vec<String> = defs
        .get(def)
        .and_then(|value| value.get("required"))
        .and_then(Value::as_array)
        .expect("the definition has `required`")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    keys.sort_unstable();
    keys
}

/// The `Step` kind enum, sorted.
fn step_kinds(defs: &Map<String, Value>) -> Vec<String> {
    let mut kinds: Vec<String> = defs
        .get("Step")
        .and_then(|step| step.get("properties"))
        .and_then(|properties| properties.get("kind"))
        .and_then(|kind| kind.get("enum"))
        .and_then(Value::as_array)
        .expect("the `Step` kind enum")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    kinds.sort_unstable();
    kinds
}

/// Assert that the schema documents every field of `value` as a property of `def`.
fn assert_documented(defs: &Map<String, Value>, def: &str, value: &Value) {
    let properties = defs
        .get(def)
        .and_then(|definition| definition.get("properties"))
        .and_then(Value::as_object)
        .expect("the definition documents properties");
    for key in value.as_object().expect("a serialized object").keys() {
        assert!(
            properties.contains_key(key),
            "the schema documents `{def}.{key}`"
        );
    }
}

/// Assert that every serialized step and selector key is documented.
fn assert_steps_documented(defs: &Map<String, Value>, scenario: &Value) {
    let steps = scenario
        .get("steps")
        .and_then(Value::as_array)
        .expect("the scenario has steps");
    for step in steps {
        assert_documented(defs, "Step", step);
        if let Some(selector) = step.get("selector") {
            assert_documented(defs, "Selector", selector);
        }
    }
}
