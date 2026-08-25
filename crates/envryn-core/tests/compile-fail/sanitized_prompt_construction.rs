// Proves AI-INV-003 is a compile error, not a review miss: nothing outside
// `envryn_core::ai::gateway` can construct a `SanitizedPrompt`, because its
// field is private to that module. If someone makes the field `pub`, this
// file starts compiling and `tests/sanitized_prompt_encapsulation.rs` fails.
use envryn_core::ai::gateway::SanitizedPrompt;

fn main() {
    let _forbidden = SanitizedPrompt("attacker-controlled text".to_string());
}
