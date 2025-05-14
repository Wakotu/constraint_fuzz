use crate::feedback::branches::constraints::Constraint;

use color_eyre::eyre::Result;

/**
 * This module is used to get function call chain from entry to constraints (inter-procedural analysis)
 */

pub type FuncChain = Vec<String>;

fn get_end_func(cons: &Constraint) -> Result<String> {
    let func_sig = cons.get_func_sig();
    extract_func_name_from_sig(func_sig).ok_or_else(|| {
        eyre::eyre!(
            "Failed to extract function name from signature: {}",
            func_sig
        )
    })
}
/// Extracts the function name from a function signature string.
/// Example: "void foo(int a, int b)" -> "foo"
fn extract_func_name_from_sig(sig: &str) -> Option<String> {
    // Find the part before the first '('
    let before_paren = sig.split('(').next()?;
    // Split by whitespace and take the last part (the function name)
    let name = before_paren.split_whitespace().last()?;
    Some(name.to_string())
}

/**
 * get executions
 */

fn get_related_executions(cons: &Constraint) -> Result<Vec<String>> {
    todo!()
}

pub fn get_func_chain(cons: &Constraint) -> Result<Vec<FuncChain>> {
    todo!()
}
