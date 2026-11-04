//! Private accounting for one charged asynchronous tool call. This is not an
//! artifact proof or a permission ticket. Every phase must re-enter the broker;
//! Runtime must also recheck current tool trust, held files and its task record.
use super::*;

#[cfg(test)]
mod tests;

// No Clone/Serde/public constructor. Only a successful, current, charged chain
// can create this record; it cannot change the original operation or identity.
pub(crate) struct ToolCallClaim {
    session: BrokerSession,
    identity: CommandIdentity,
    operation: CapabilityOperation,
    charged: Vec<(BrokerSession, u32)>,
}
impl ToolCallClaim {
    pub(crate) fn identity(&self) -> &CommandIdentity {
        &self.identity
    }
}
impl CapabilityBroker {
    pub(crate) fn claim_tool_call(
        &mut self,
        session: &BrokerSession,
        operation: &CapabilityOperation,
        at: f64,
    ) -> Result<ToolCallClaim, ErrorCode> {
        if !matches!(operation, CapabilityOperation::ToolExecute { .. }) {
            return Err(ErrorCode::CapabilityUndeclared);
        }
        let (identity, chain) = self.invocation_chain(session, operation, at)?;
        let charged = chain
            .into_iter()
            .map(|hop| {
                let binding = self.bindings.get_mut(&hop.id).unwrap();
                binding.calls += 1;
                (hop, binding.calls)
            })
            .collect();
        Ok(ToolCallClaim {
            session: session.clone(),
            identity,
            operation: operation.clone(),
            charged,
        })
    }
    pub(crate) fn invoke_claimed_tool<T>(
        &self,
        claim: &ToolCallClaim,
        at: f64,
        adapter: impl FnOnce(&CommandIdentity) -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        self.check_session(&claim.session, at)?;
        let binding = &self.bindings[&claim.session.id];
        let mut chain = binding.parents.clone();
        chain.push(claim.session.clone());
        if binding.identity != claim.identity
            || chain.len() != claim.charged.len()
            || chain
                .iter()
                .zip(&claim.charged)
                .any(|(hop, (charged, _))| hop.id != charged.id)
        {
            return Err(ErrorCode::SessionInvalid);
        }
        for (hop, ordinal) in &claim.charged {
            self.check_operation_scope(hop, &claim.operation)?;
            let binding = &self.bindings[&hop.id];
            let grant = self.policies[&binding.identity].grant.as_ref().unwrap();
            // An already charged last call may continue through current checks.
            // It cannot manufacture a fresh call or survive a changed budget.
            if *ordinal == 0 || binding.calls < *ordinal || binding.calls > grant.max_calls {
                return Err(ErrorCode::BudgetExceeded);
            }
        }
        adapter(&claim.identity)
    }
}
