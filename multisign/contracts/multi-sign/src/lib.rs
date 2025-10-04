#![no_std]

mod test;

use soroban_sdk::{
    contract, contractimpl, contracterror, contracttype, contractmeta,
    symbol_short, Address, BytesN, Env, Vec, Symbol, Map, token
};

contractmeta!(
    key = "description",
    val = "Secure Multi-signature Wallet Contract"
);

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MultisigError {
    Unauthorized = 1,
    InvalidThreshold = 2,
    TransactionNotFound = 3,
    TransactionExecuted = 4,
    AlreadyApproved = 5,
    InsufficientApprovals = 6,
    InvalidOwner = 7,
    ArithmeticError = 8,
    DuplicateOwner = 9,
    AlreadyInitialized = 10,
    InvalidAmount = 11,
    InvalidAddress = 12,
    TokenTransferFailed = 13,
}

#[contract]
pub struct MultisigContract;

#[contracttype]
#[derive(Clone, Debug)]
pub struct Transaction {
    pub to: Address,
    pub amount: i128,
    pub token: Address,
    pub data: BytesN<32>,
    pub executed: bool,
    pub approvals: u32,
    pub submitter: Address,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MultisigConfig {
    pub owners: Vec<Address>,
    pub required_approvals: u32,
    pub transaction_count: u32,
}

const CONFIG_KEY: Symbol = symbol_short!("config");
const TX_KEY: Symbol = symbol_short!("tx");
const APPROVAL_KEY: Symbol = symbol_short!("approval");

#[contractimpl]
impl MultisigContract {

    pub fn initialize(
        env: Env,
        owners: Vec<Address>,
        required_approvals: u32,
    ) -> Result<(), MultisigError> {
        if env.storage().persistent().has(&CONFIG_KEY) {
            return Err(MultisigError::AlreadyInitialized);
        }

        if owners.is_empty() {
            return Err(MultisigError::InvalidOwner);
        }
        
        if required_approvals == 0 || required_approvals > owners.len() as u32 {
            return Err(MultisigError::InvalidThreshold);
        }

        let mut seen = Map::new(&env);
        for owner in owners.iter() {
            if seen.contains_key(owner.clone()) {
                return Err(MultisigError::DuplicateOwner);
            }
            seen.set(owner.clone(), true);
        }

        let config = MultisigConfig {
            owners: owners.clone(),
            required_approvals,
            transaction_count: 0,
        };

        env.storage().persistent().set(&CONFIG_KEY, &config);
        
        env.events().publish((symbol_short!("init"), owners.len(), required_approvals), ());
        
        Ok(())
    }

    fn verify_owner(env: &Env, caller: &Address) -> Result<(), MultisigError> {
        let config: MultisigConfig = env.storage().persistent().get(&CONFIG_KEY)
            .ok_or(MultisigError::Unauthorized)?;
        
        if !config.owners.contains(caller) {
            return Err(MultisigError::Unauthorized);
        }
        
        Ok(())
    }

    fn get_config(env: &Env) -> Result<MultisigConfig, MultisigError> {
        env.storage().persistent().get(&CONFIG_KEY)
            .ok_or(MultisigError::Unauthorized)
    }

    fn validate_transaction_inputs(
        _to: &Address,
        amount: i128,
        _token: &Address,
    ) -> Result<(), MultisigError> {
        if amount <= 0 {
            return Err(MultisigError::InvalidAmount);
        }
        
        Ok(())
    }

 
    pub fn submit_transaction(
        env: Env,
        caller: Address,
        to: Address,
        amount: i128,
        token: Address,
        data: BytesN<32>,
    ) -> Result<u32, MultisigError> {
        caller.require_auth();
        Self::verify_owner(&env, &caller)?;
        
        Self::validate_transaction_inputs(&to, amount, &token)?;
        
        let mut config = Self::get_config(&env)?;
        
        let new_count = config.transaction_count.checked_add(1)
            .ok_or(MultisigError::ArithmeticError)?;
        
        let transaction = Transaction {
            to: to.clone(),
            amount,
            token: token.clone(),
            data,
            executed: false,
            approvals: 1, 
            submitter: caller.clone(),
        };

        config.transaction_count = new_count;
        env.storage().persistent().set(&CONFIG_KEY, &config);
        
        env.storage().persistent().set(&(TX_KEY, new_count), &transaction);
        
        let mut approvals = Vec::new(&env);
        approvals.push_back(caller.clone());
        env.storage().persistent().set(&(APPROVAL_KEY, new_count), &approvals);

        env.events().publish(
            (symbol_short!("submit"), new_count),
            (caller, to, amount, token)
        );

        Ok(new_count)
    }


    pub fn approve_transaction(
        env: Env, 
        caller: Address,
        transaction_id: u32
    ) -> Result<(), MultisigError> {
        caller.require_auth();
        Self::verify_owner(&env, &caller)?;

        let tx_key = (TX_KEY, transaction_id);
        let mut transaction: Transaction = env.storage().persistent().get(&tx_key)
            .ok_or(MultisigError::TransactionNotFound)?;

        if transaction.executed {
            return Err(MultisigError::TransactionExecuted);
        }

        let approval_key = (APPROVAL_KEY, transaction_id);
        let mut approvals: Vec<Address> = env.storage().persistent().get(&approval_key)
            .unwrap_or_else(|| Vec::new(&env));

        if approvals.contains(&caller) {
            return Err(MultisigError::AlreadyApproved);
        }

        let new_approvals = transaction.approvals.checked_add(1)
            .ok_or(MultisigError::ArithmeticError)?;
        
        transaction.approvals = new_approvals;
        env.storage().persistent().set(&tx_key, &transaction);
        
        approvals.push_back(caller.clone());
        env.storage().persistent().set(&approval_key, &approvals);

        env.events().publish(
            (symbol_short!("approve"), transaction_id),
            (caller, new_approvals)
        );

        Ok(())
    }

    pub fn execute_transaction(
        env: Env, 
        caller: Address,
        transaction_id: u32
    ) -> Result<(), MultisigError> {
        caller.require_auth();
        Self::verify_owner(&env, &caller)?;

        let tx_key = (TX_KEY, transaction_id);
        let mut transaction: Transaction = env.storage().persistent().get(&tx_key)
            .ok_or(MultisigError::TransactionNotFound)?;

        if transaction.executed {
            return Err(MultisigError::TransactionExecuted);
        }

        let config = Self::get_config(&env)?;

        if transaction.approvals < config.required_approvals {
            return Err(MultisigError::InsufficientApprovals);
        }

        transaction.executed = true;
        env.storage().persistent().set(&tx_key, &transaction);

        let token_client = token::Client::new(&env, &transaction.token);
        

        match token_client.try_transfer(
            &env.current_contract_address(),
            &transaction.to,
            &transaction.amount
        ) {
            Ok(_) => {
                env.events().publish(
                    (symbol_short!("execute"), transaction_id),
                    (caller, transaction.to.clone(), transaction.amount, transaction.token.clone())
                );
                Ok(())
            },
            Err(_) => {
                transaction.executed = false;
                env.storage().persistent().set(&tx_key, &transaction);
                Err(MultisigError::TokenTransferFailed)
            }
        }
    }


    
    pub fn get_transaction(
        env: Env,
        caller: Address,
        transaction_id: u32
    ) -> Result<Transaction, MultisigError> {
        caller.require_auth();
        Self::verify_owner(&env, &caller)?;
        env.storage().persistent().get(&(TX_KEY, transaction_id))
            .ok_or(MultisigError::TransactionNotFound)
    }

    pub fn is_owner(env: Env, address: Address) -> Result<bool, MultisigError> {
        let config = Self::get_config(&env)?;
        Ok(config.owners.contains(&address))
    }

    pub fn get_approvals(
        env: Env,
        caller: Address,
        transaction_id: u32
    ) -> Result<Vec<Address>, MultisigError> {
        caller.require_auth();
        Self::verify_owner(&env, &caller)?;
        env.storage().persistent().get(&(APPROVAL_KEY, transaction_id))
            .ok_or(MultisigError::TransactionNotFound)
    }

}
