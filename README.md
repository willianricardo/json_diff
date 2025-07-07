json_diff

A lightweight Rust library for computing and applying JSON diffs. It allows you to compare two JSON Values, generate a delta representing the changes, apply those changes to an original JSON, and revert them if needed.

🚀 Features
	•	Compute Differences: Compare two serde_json::Value instances and produce a Delta mapping JSON paths to changes.
	•	Change Types: Support for Add, Remove, and Modify operations.
	•	Apply Deltas: Apply a computed delta to an original JSON object to obtain the modified version.
	•	Revert Changes: Revert applied deltas to return to the original JSON state.
	•	Ordered Keys: Uses BTreeMap to keep keys in the delta sorted for consistent output.

📦 Installation

Add the following to your Cargo.toml:

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
json_diff = { git = "https://github.com/willianricardo/json_diff", branch = "main" }
```

Then run `cargo build` to install the dependencies.

🔧 Usage
```rust
use serde_json::json;
use json_diff::{diff, apply, revert};

fn main() {
    let original = json!({
        "user": { "name": "Alice", "age": 30 },
        "active": true
    });

    let updated = json!({
        "user": { "name": "Alice", "age": 31 },
        "active": false,
        "role": "admin"
    });

    // Compute the diff between the two JSON values
    let delta = diff(&original, &updated);
    println!("Delta: {:#?}", delta);

    // Apply the delta to the original JSON
    let applied = apply(&original, &delta);
    assert_eq!(applied, updated);

    // Revert the changes
    let reverted = revert(&updated, &delta);
    assert_eq!(reverted, original);
}
```

API Reference

enum Change

```rust
pub enum Change {
    Add(Value),
    Remove(Value),
    Modify { old: Value, new: Value },
}
```

Represents a single change in the JSON structure.
	•	Add(value): A value was added at the given path.
	•	Remove(value): A value was removed from the given path.
	•	Modify { old, new }: A value was changed from old to new.

type Delta

```rust
pub type Delta = BTreeMap<String, Change>;
```

A map from JSON paths (dot-separated keys) to Change instances.

```rust
fn diff(before: &Value, after: &Value) -> Delta
```

Compute the delta between two JSON values.

```rust
fn apply(original: &Value, delta: &Delta) -> Value
```

Apply a delta to the original JSON value, returning a new Value with changes applied.

```rust
fn revert(original: &Value, delta: &Delta) -> Value
```

Revert a delta on a JSON value, returning the previous state.

🤝 Contributing

Contributions, issues, and feature requests are welcome!
	1.	Fork the repository
	2.	Create your feature branch (git checkout -b feature/YourFeature)
	3.	Commit your changes (git commit -m 'Add some feature')
	4.	Push to the branch (git push origin feature/YourFeature)
	5.	Open a Pull Request

Please ensure your code adheres to Rust’s formatting conventions (e.g., rustfmt).

📄 License

This project is licensed under the MIT License. See the LICENSE file for details.

⸻

Happy diffing!
