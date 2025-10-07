#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod v6psp20piggybank {
    use ink::storage::Mapping;
    use ink::primitives::{H160, U256};
    use ink::env::call::{build_call, ExecutionInput, Selector};
    use ink::env::DefaultEnvironment;

    /// Event emitted when a deposit occurs
    #[ink(event)]
    pub struct Deposit {
        #[ink(topic)]
        owner: H160,
        amount: Balance,
        total: Balance,
    }

    /// Event emitted when a withdrawal occurs
    #[ink(event)]
    pub struct Withdrawal {
        #[ink(topic)]
        owner: H160,
        amount: Balance,
        remaining: Balance,
    }

    /// Event emitted when piggy bank is broken (all funds withdrawn)
    #[ink(event)]
    pub struct PiggyBankBroken {
        #[ink(topic)]
        owner: H160,
        amount: Balance,
    }

    /// Event emitted when goal is reached
    #[ink(event)]
    pub struct GoalReached {
        #[ink(topic)]
        owner: H160,
        goal: Balance,
    }

    #[derive(Debug, PartialEq, Eq)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        InsufficientBalance,
        GoalNotReached,
        WithdrawalTooEarly,
        Unauthorized,
        ZeroAmount,
        TokenTransferFailed,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct V6psp20piggybank {
        /// Token contract address for CallBuilder (H160 for ink! v6)
        token_address: H160,
        /// Mapping from owner to their balance
        balances: Mapping<H160, Balance>,
        /// Mapping from owner to their savings goal
        goals: Mapping<H160, Balance>,
        /// Mapping from owner to their lock time (timestamp)
        lock_times: Mapping<H160, u64>,
        /// Contract owner
        owner: H160,
    }

    impl V6psp20piggybank {
        /// Constructor that initializes the piggy bank with a token contract address
        #[ink(constructor)]
        pub fn new(token_address: H160) -> Self {
            Self {
                token_address,
                balances: Mapping::default(),
                goals: Mapping::default(),
                lock_times: Mapping::default(),
                owner: Self::env().caller(),
            }
        }

        /// Deposit tokens into the piggy bank (requires prior approval)
        #[ink(message)]
        pub fn deposit(&mut self, amount: Balance) -> Result<()> {
            let caller = self.env().caller();

            if amount == 0 {
                return Err(Error::ZeroAmount);
            }

            // Convert AccountId to H160 for cross-contract call
            let contract_h160: H160 = self.convert_account_to_h160(self.env().account_id());

            // Use CallBuilder to call transfer_from on the token contract
            build_call::<DefaultEnvironment>()
                .call(self.token_address)
                .transferred_value(U256::zero())
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer_from")))
                        .push_arg(caller)
                        .push_arg(contract_h160)
                        .push_arg(amount)
                )
                .returns::<core::result::Result<(), ()>>()
                .try_invoke()
                .map_err(|_| Error::TokenTransferFailed)?
                .map_err(|_| Error::TokenTransferFailed)?;

            let current_balance = self.balance_of(caller);
            let new_balance = current_balance.saturating_add(amount);
            self.balances.insert(caller, &new_balance);

            self.env().emit_event(Deposit {
                owner: caller,
                amount,
                total: new_balance,
            });

            // Check if goal is reached
            if let Some(goal) = self.goals.get(caller) {
                if new_balance >= goal {
                    self.env().emit_event(GoalReached {
                        owner: caller,
                        goal,
                    });
                }
            }

            Ok(())
        }

        /// Set a savings goal
        #[ink(message)]
        pub fn set_goal(&mut self, goal: Balance) -> Result<()> {
            let caller = self.env().caller();
            self.goals.insert(caller, &goal);
            Ok(())
        }

        /// Set a lock time (timestamp in milliseconds) - funds cannot be withdrawn until this time
        #[ink(message)]
        pub fn set_lock_time(&mut self, lock_time: u64) -> Result<()> {
            let caller = self.env().caller();
            self.lock_times.insert(caller, &lock_time);
            Ok(())
        }

        /// Withdraw a specific amount
        #[ink(message)]
        pub fn withdraw(&mut self, amount: Balance) -> Result<()> {
            let caller = self.env().caller();
            let current_balance = self.balance_of(caller);

            if amount == 0 {
                return Err(Error::ZeroAmount);
            }

            if current_balance < amount {
                return Err(Error::InsufficientBalance);
            }

            // Check lock time
            if let Some(lock_time) = self.lock_times.get(caller) {
                if self.env().block_timestamp() < lock_time {
                    return Err(Error::WithdrawalTooEarly);
                }
            }

            let new_balance = current_balance.saturating_sub(amount);
            self.balances.insert(caller, &new_balance);

            // Use CallBuilder to call transfer on the token contract
            build_call::<DefaultEnvironment>()
                .call(self.token_address)
                .transferred_value(U256::zero())
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer")))
                        .push_arg(caller)
                        .push_arg(amount)
                )
                .returns::<core::result::Result<(), ()>>()
                .try_invoke()
                .map_err(|_| Error::TokenTransferFailed)?
                .map_err(|_| Error::TokenTransferFailed)?;

            self.env().emit_event(Withdrawal {
                owner: caller,
                amount,
                remaining: new_balance,
            });

            Ok(())
        }

        /// Break the piggy bank - withdraw all funds
        #[ink(message)]
        pub fn break_piggy_bank(&mut self) -> Result<()> {
            let caller = self.env().caller();
            let balance = self.balance_of(caller);

            if balance == 0 {
                return Err(Error::InsufficientBalance);
            }

            // Check lock time
            if let Some(lock_time) = self.lock_times.get(caller) {
                if self.env().block_timestamp() < lock_time {
                    return Err(Error::WithdrawalTooEarly);
                }
            }

            self.balances.remove(caller);
            self.goals.remove(caller);
            self.lock_times.remove(caller);

            // Use CallBuilder to call transfer on the token contract
            build_call::<DefaultEnvironment>()
                .call(self.token_address)
                .transferred_value(U256::zero())
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer")))
                        .push_arg(caller)
                        .push_arg(balance)
                )
                .returns::<core::result::Result<(), ()>>()
                .try_invoke()
                .map_err(|_| Error::TokenTransferFailed)?
                .map_err(|_| Error::TokenTransferFailed)?;

            self.env().emit_event(PiggyBankBroken {
                owner: caller,
                amount: balance,
            });

            Ok(())
        }

        /// Withdraw if goal is reached
        #[ink(message)]
        pub fn withdraw_if_goal_reached(&mut self, amount: Balance) -> Result<()> {
            let caller = self.env().caller();
            let current_balance = self.balance_of(caller);

            if let Some(goal) = self.goals.get(caller) {
                if current_balance < goal {
                    return Err(Error::GoalNotReached);
                }
            }

            self.withdraw(amount)
        }

        /// Returns the balance of the given account
        #[ink(message)]
        pub fn balance_of(&self, owner: H160) -> Balance {
            self.balances.get(owner).unwrap_or(0)
        }

        /// Returns the savings goal of the given account
        #[ink(message)]
        pub fn goal_of(&self, owner: H160) -> Balance {
            self.goals.get(owner).unwrap_or(0)
        }

        /// Returns the lock time of the given account
        #[ink(message)]
        pub fn lock_time_of(&self, owner: H160) -> u64 {
            self.lock_times.get(owner).unwrap_or(0)
        }

        /// Returns whether the goal is reached for an account
        #[ink(message)]
        pub fn is_goal_reached(&self, owner: H160) -> bool {
            let balance = self.balance_of(owner);
            if let Some(goal) = self.goals.get(owner) {
                balance >= goal
            } else {
                false
            }
        }

        /// Returns the contract owner
        #[ink(message)]
        pub fn owner(&self) -> H160 {
            self.owner
        }

        /// Returns the token contract address
        #[ink(message)]
        pub fn token_address(&self) -> H160 {
            self.token_address
        }

        /// Get token balance of this contract in the PSP20 token
        #[ink(message)]
        pub fn token_balance(&self) -> Balance {
            let contract_h160 = self.convert_account_to_h160(self.env().account_id());

            // Use CallBuilder to call balance_of on the token contract
            build_call::<DefaultEnvironment>()
                .call(self.token_address)
                .transferred_value(U256::zero())
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("balance_of")))
                        .push_arg(contract_h160)
                )
                .returns::<Balance>()
                .try_invoke()
                .unwrap_or(Ok(0))
                .unwrap_or(0)
        }

        /// Helper function to convert AccountId to H160
        fn convert_account_to_h160(&self, account: AccountId) -> H160 {
            Self::convert_account_id_to_h160(account)
        }

        /// Static helper function to convert AccountId to H160
        fn convert_account_id_to_h160(account: AccountId) -> H160 {
            let account_bytes = <AccountId as AsRef<[u8]>>::as_ref(&account);
            let mut h160_bytes = [0u8; 20];
            h160_bytes.copy_from_slice(&account_bytes[..20]);
            H160::from(h160_bytes)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::test;

        fn get_default_accounts() -> test::DefaultAccounts {
            test::default_accounts()
        }

        fn get_bob() -> H160 {
            H160::from([2u8; 20])
        }

        fn create_mock_token() -> H160 {
            // Create a mock token contract address for testing (H160 for ink! v6)
            H160::from([0x01; 20])
        }

        #[ink::test]
        fn new_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let token_address = create_mock_token();
            let piggy_bank = V6psp20piggybank::new(token_address);

            assert_eq!(piggy_bank.balance_of(accounts.alice), 0);
            assert_eq!(piggy_bank.owner(), accounts.alice);
        }

        #[ink::test]
        fn set_goal_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let token_address = create_mock_token();
            let mut piggy_bank = V6psp20piggybank::new(token_address);

            assert!(piggy_bank.set_goal(1000).is_ok());
            assert_eq!(piggy_bank.goal_of(accounts.alice), 1000);
        }

        #[ink::test]
        fn set_lock_time_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let token_address = create_mock_token();
            let mut piggy_bank = V6psp20piggybank::new(token_address);

            assert!(piggy_bank.set_lock_time(1000000).is_ok());
            assert_eq!(piggy_bank.lock_time_of(accounts.alice), 1000000);
        }

        #[ink::test]
        fn goal_reached_logic_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let token_address = create_mock_token();
            let mut piggy_bank = V6psp20piggybank::new(token_address);

            piggy_bank.set_goal(100).unwrap();
            assert!(!piggy_bank.is_goal_reached(accounts.alice));

            // Manually set balance for testing
            piggy_bank.balances.insert(accounts.alice, &100);
            assert!(piggy_bank.is_goal_reached(accounts.alice));
        }

        #[ink::test]
        fn multiple_users_work() {
            let accounts = get_default_accounts();
            let bob = get_bob();

            let token_address = create_mock_token();
            let mut piggy_bank = V6psp20piggybank::new(token_address);

            // Alice sets goal
            test::set_caller(accounts.alice);
            piggy_bank.set_goal(1000).unwrap();

            // Bob sets different goal
            test::set_caller(bob);
            piggy_bank.set_goal(2000).unwrap();

            assert_eq!(piggy_bank.goal_of(accounts.alice), 1000);
            assert_eq!(piggy_bank.goal_of(bob), 2000);
        }
    }

}
