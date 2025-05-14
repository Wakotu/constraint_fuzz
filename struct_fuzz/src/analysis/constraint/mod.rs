use crate::feedback::branches::constraints::Constraint;
use color_eyre::eyre::Result;

pub mod inter;

/**
 * This module is used to get dataflow information of a specified constraint.
 */

// define Dataflow info for constraitns: a set of Statements which results in variables in that Constraint

pub type Statement = String; // TODO: define a proper type for statements
pub type ConsDFInfo = Vec<Statement>;

pub fn analyze_constraint(cons: &Constraint) -> Result {
    let mut dataflow_info = Vec::new();
}

// TODO: add unit tests
