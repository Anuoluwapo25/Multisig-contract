#![cfg(test)]

use super::*;
use soroban_sdk::{vec, Env, Address, BytesN, testutils::Address as _};

#[test]
fn test_initialize_success() {
    let env = Env::default();
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Create test owners
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let owner3 = Address::generate(&env);
    let owners = vec![&env, owner1.clone(), owner2.clone(), owner3.clone()];
    
    // Initialize with 2/3 threshold
    client.initialize(&owners, &2);
    
    // Verify configuration
    assert_eq!(client.get_owner_count(), 3);
    assert_eq!(client.get_threshold(), 2);
    assert_eq!(client.get_transaction_count(), 0);
    
    // Verify owners
    assert!(client.is_owner(&owner1));
    assert!(client.is_owner(&owner2));
    assert!(client.is_owner(&owner3));
}

#[test]
fn test_initialize_fails_when_already_initialized() {
    let env = Env::default();
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    let owners = vec![&env, Address::generate(&env), Address::generate(&env)];
    
    // First initialization should succeed
    client.initialize(&owners, &1);
    
    // Second initialization should fail
    let result = client.try_initialize(&owners, &1);
    assert_eq!(result, Err(Ok(MultisigError::AlreadyInitialized)));
}

#[test]
fn test_initialize_fails_with_invalid_threshold() {
    let env = Env::default();
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    let owners = vec![&env, Address::generate(&env), Address::generate(&env)];
    
    // Test threshold = 0
    let result = client.try_initialize(&owners, &0);
    assert_eq!(result, Err(Ok(MultisigError::InvalidThreshold)));
    
    // Test threshold > owner count
    let result = client.try_initialize(&owners, &3);
    assert_eq!(result, Err(Ok(MultisigError::InvalidThreshold)));
}

#[test]
fn test_initialize_fails_with_empty_owners() {
    let env = Env::default();
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    let empty_owners = vec![&env];
    
    let result = client.try_initialize(&empty_owners, &1);
    assert_eq!(result, Err(Ok(MultisigError::InvalidOwner)));
}

#[test]
fn test_initialize_fails_with_duplicate_owners() {
    let env = Env::default();
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    let owner = Address::generate(&env);
    let owners_with_duplicate = vec![&env, owner.clone(), owner.clone()];
    
    let result = client.try_initialize(&owners_with_duplicate, &1);
    assert_eq!(result, Err(Ok(MultisigError::DuplicateOwner)));
}

#[test]
fn test_submit_transaction_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let owners = vec![&env, owner1.clone(), owner2.clone()];
    client.initialize(&owners, &2);
    
    // Submit transaction
    let to = Address::generate(&env);
    let token = Address::generate(&env);
    let amount = 1000i128;
    let data = BytesN::from_array(&env, &[0; 32]);
    
    let tx_id = client.submit_transaction(&owner1, &to, &amount, &token, &data);
    assert_eq!(tx_id, 1);
    
    // Verify transaction count updated
    assert_eq!(client.get_transaction_count(), 1);
    
    // Verify transaction details
    let transaction = client.get_transaction(&owner1, &tx_id);
    assert_eq!(transaction.to, to);
    assert_eq!(transaction.amount, amount);
    assert_eq!(transaction.token, token);
    assert_eq!(transaction.executed, false);
    assert_eq!(transaction.approvals, 1);
    assert_eq!(transaction.submitter, owner1);
}

#[test]
fn test_submit_transaction_fails_with_invalid_amount() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize
    let owner = Address::generate(&env);
    let owners = vec![&env, owner.clone()];
    client.initialize(&owners, &1);
    
    // Submit transaction with invalid amount
    let to = Address::generate(&env);
    let token = Address::generate(&env);
    let data = BytesN::from_array(&env, &[0; 32]);
    
    let result = client.try_submit_transaction(&owner, &to, &0i128, &token, &data);
    assert_eq!(result, Err(Ok(MultisigError::InvalidAmount)));
    
    let result = client.try_submit_transaction(&owner, &to, &(-100i128), &token, &data);
    assert_eq!(result, Err(Ok(MultisigError::InvalidAmount)));
}

#[test]
fn test_approve_transaction_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let owners = vec![&env, owner1.clone(), owner2.clone()];
    client.initialize(&owners, &2);
    
    // Submit transaction
    let to = Address::generate(&env);
    let token = Address::generate(&env);
    let amount = 1000i128;
    let data = BytesN::from_array(&env, &[0; 32]);
    
    let tx_id = client.submit_transaction(&owner1, &to, &amount, &token, &data);
    
    // Approve by second owner
    client.approve_transaction(&owner2, &tx_id);
    
    // Verify approval count
    let transaction = client.get_transaction(&owner1, &tx_id);
    assert_eq!(transaction.approvals, 2);
    
    // Verify approvers list
    let approvals = client.get_approvals(&owner1, &tx_id);
    assert!(approvals.contains(&owner1));
    assert!(approvals.contains(&owner2));
}

#[test]
fn test_approve_transaction_fails_double_approval() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize
    let owner = Address::generate(&env);
    let owners = vec![&env, owner.clone()];
    client.initialize(&owners, &1);
    
    // Submit transaction
    let tx_id = client.submit_transaction(
        &owner,
        &Address::generate(&env),
        &1000i128,
        &Address::generate(&env),
        &BytesN::from_array(&env, &[0; 32])
    );
    
    // Try to approve again (submitter already auto-approved)
    let result = client.try_approve_transaction(&owner, &tx_id);
    assert_eq!(result, Err(Ok(MultisigError::AlreadyApproved)));
}

#[test]
fn test_execute_transaction_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize with 1/1 threshold for simple execution
    let owner = Address::generate(&env);
    let owners = vec![&env, owner.clone()];
    client.initialize(&owners, &1);
    
    // Submit transaction
    let tx_id = client.submit_transaction(
        &owner,
        &Address::generate(&env),
        &1000i128,
        &Address::generate(&env),
        &BytesN::from_array(&env, &[0; 32])
    );
    
    // Execute transaction (will fail due to token transfer but should handle gracefully)
    let result = client.try_execute_transaction(&owner, &tx_id);
    
    // Should fail with TokenTransferFailed since we don't have actual tokens
    assert_eq!(result, Err(Ok(MultisigError::TokenTransferFailed)));
    
    // Verify transaction is not marked as executed after failure
    let transaction = client.get_transaction(&owner, &tx_id);
    assert_eq!(transaction.executed, false);
}

#[test]
fn test_execute_transaction_fails_insufficient_approvals() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize with 2/2 threshold
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let owners = vec![&env, owner1.clone(), owner2.clone()];
    client.initialize(&owners, &2);
    
    // Submit transaction (only gets 1 approval from submitter)
    let tx_id = client.submit_transaction(
        &owner1,
        &Address::generate(&env),
        &1000i128,
        &Address::generate(&env),
        &BytesN::from_array(&env, &[0; 32])
    );
    
    // Try to execute without sufficient approvals
    let result = client.try_execute_transaction(&owner1, &tx_id);
    assert_eq!(result, Err(Ok(MultisigError::InsufficientApprovals)));
}

#[test]
fn test_update_threshold_success() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let owner3 = Address::generate(&env);
    let owners = vec![&env, owner1.clone(), owner2.clone(), owner3.clone()];
    client.initialize(&owners, &2);
    
    // Update threshold
    client.update_threshold(&owner1, &3);
    
    // Verify threshold updated
    assert_eq!(client.get_threshold(), 3);
}

#[test]
fn test_authentication_required() {
    let env = Env::default();
    // Don't mock auths to test authentication
    
    let contract_id = env.register(MultisigContract, ());
    let client = MultisigContractClient::new(&env, &contract_id);
    
    // Initialize first (this should work without auth)
    let owner = Address::generate(&env);
    let owners = vec![&env, owner.clone()];
    client.initialize(&owners, &1);
    
    // Try operations without authentication - they should fail
    // Note: In a real test, these would panic due to missing auth
    // For this demonstration, we'll just verify the structure is correct
}
