use std::path::PathBuf;

use crate::feedback::branches::constraints::Constraint;
use color_eyre::eyre::Result;

pub mod inter;

/**
 * This module is used to get dataflow information of a specified constraint.
 */

// define Dataflow info for constraitns: a set of Statements which results in variables in that Constraint

pub type Statement = String; // TODO: define a proper type for statements
pub type ConsDFInfo = Vec<Statement>;

pub struct ConsDFBuilder {
    cons: Constraint,
    work_dir: PathBuf,
}

impl ConsDFBuilder {
    pub fn new(cons: Constraint, work_dir: PathBuf) -> Self {
        Self { cons, work_dir }
    }
    pub fn analyze_constraint(&self, cons: &Constraint) -> Result<ConsDFInfo> {
        todo!()
    }

    pub fn build(&self, cons: &Constraint) -> Result<ConsDFInfo> {
        self.analyze_constraint(cons)
    }
}

// TODO: add unit tests
