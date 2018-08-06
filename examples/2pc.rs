//! This module implements a subset of the two phase commit specification presented in the paper
//! ["Consensus on Transaction Commit"](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/tr-2003-96.pdf)
//! by Jim Gray and Leslie Lamport.

#[macro_use]
extern crate clap;
extern crate stateright;

use clap::*;
use stateright::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::hash::Hash;

struct TwoPhaseSys<R> { pub rms: BTreeSet<R> }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TwoPhaseState<R> {
    rm_state: BTreeMap<R, RmState>,
    tm_state: TmState,
    tm_prepared: BTreeSet<R>,
    msgs: BTreeSet<Message<R>>
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
enum Message<R> { Prepared { rm: R }, Commit, Abort }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum RmState { Working, Prepared, Committed, Aborted }

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum TmState { Init, Committed, Aborted }

impl<R: Clone + Debug + Eq + Hash + Ord> TwoPhaseSys<R> {
    fn tm_rcv_prepared(&self, rm: &R, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.tm_state == TmState::Init
                && state.msgs.contains(&Message::Prepared { rm: rm.clone() }) {
            let mut result = state.clone();
            result.tm_prepared.insert(rm.clone());
            results.push(("TM got prepared msg", result));
        }
    }
    fn tm_commit(&self, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.tm_state == TmState::Init
                && state.tm_prepared == self.rms {
            let mut result = state.clone();
            result.tm_state = TmState::Committed;
            result.msgs.insert(Message::Commit);
            results.push(("TM was able to commit and has informed RMs", result));
        }
    }
    fn tm_abort(&self, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.tm_state == TmState::Init {
            let mut result = state.clone();
            result.tm_state = TmState::Aborted;
            result.msgs.insert(Message::Abort);
            results.push(("TM chose to abort", result));
        }
    }
    fn rm_prepare(&self, rm: &R, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.rm_state.get(rm) == Some(&RmState::Working) {
            let mut result = state.clone();
            result.rm_state.insert(rm.clone(), RmState::Prepared);
            result.msgs.insert(Message::Prepared { rm: rm.clone() });
            results.push(("RM is preparing", result));
        }
    }
    fn rm_choose_to_abort(&self, rm: &R, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.rm_state.get(rm) == Some(&RmState::Working) {
            let mut result = state.clone();
            result.rm_state.insert(rm.clone(), RmState::Aborted);
            results.push(("RM is choosing to abort", result));
        }
    }
    fn rm_rcv_commit_msg(&self, rm: &R, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.msgs.contains(&Message::Commit) {
            let mut result = state.clone();
            result.rm_state.insert(rm.clone(), RmState::Committed);
            results.push(("RM is being told to commit", result));
        }
    }
    fn rm_rcv_abort_msg(&self, rm: &R, state: &TwoPhaseState<R>, results: &mut StepVec<TwoPhaseState<R>>) {
        if state.msgs.contains(&Message::Abort) {
            let mut result = state.clone();
            result.rm_state.insert(rm.clone(), RmState::Aborted);
            results.push(("RM is being told to abort", result));
        }
    }
}

impl<R: Clone + Debug + Eq + Hash + Ord> StateMachine for TwoPhaseSys<R> {
    type State = TwoPhaseState<R>;
    fn init(&self, results: &mut StepVec<Self::State>) {
        let state = TwoPhaseState {
            rm_state: self.rms.iter().map(|rm| (rm.clone(), RmState::Working)).collect(),
            tm_state: TmState::Init,
            tm_prepared: BTreeSet::new(),
            msgs: BTreeSet::new()
        };
        results.push(("init", state));
    }

    fn next(&self, state: &Self::State, results: &mut StepVec<Self::State>) {
        self.tm_commit(state, results);
        self.tm_abort(state, results);
        for rm in &self.rms {
            self.tm_rcv_prepared(rm, state, results);
            self.rm_prepare(rm, state, results);
            self.rm_choose_to_abort(rm, state, results);
            self.rm_rcv_commit_msg(rm, state, results);
            self.rm_rcv_abort_msg(rm, state, results);
        }
    }
}

fn is_consistent<R: Clone + Eq + Hash + Ord>(sys: &TwoPhaseSys<R>, state: &TwoPhaseState<R>) -> bool {
    !sys.rms.iter().any(|rm1|
        sys.rms.iter().any(|rm2|
            state.rm_state[rm1] == RmState::Aborted && state.rm_state[rm2] == RmState::Committed))
}

#[cfg(test)]
#[test]
fn can_model_2pc() {
    let mut rms = BTreeSet::new();
    for rm in 1..(5+1) {
        rms.insert(rm);
    }
    let sys = TwoPhaseSys { rms };
    let mut checker = sys.checker(KeepPaths::No, is_consistent);
    assert_eq!(
        checker.check(1_000_000),
        CheckResult::Pass);
    assert_eq!(
        checker.visited.len(),
        8832);
}

fn main() {
    let args = App::new("2pc")
        .about("model check abstract two phase commit")
        .arg(Arg::with_name("rm_count")
             .help("number of resource managers")
             .default_value("7"))
        .get_matches();

    let rm_count = value_t!(args, "rm_count", u32).expect("rm_count");
    println!("Benchmarking two phase commit with {} resource managers.", rm_count);

    let sys = TwoPhaseSys {
        rms: BTreeSet::from_iter(0..rm_count)
    };
    sys.checker(KeepPaths::Yes, is_consistent).check_and_report();
}
