#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod Token {
    use ink::prelude::vec::Vec;
    use ink::storage::Mapping;
    use ink::primitives::H160;

    /// Event emitted when a token transfer occurs
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<H160>,
        #[ink(topic)]
        to: Option<H160>,
        value: Balance,
    }

    /// Event emitted when an approval occurs
    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: H160,
        #[ink(topic)]
        spender: H160,
        value: Balance,
    }

    /// Event emitted when tokens are burned
    #[ink(event)]
    pub struct Burn {
        #[ink(topic)]
        from: H160,
        value: Balance,
    }

    /// Event emitted when contract is paused
    #[ink(event)]
    pub struct Paused {
        #[ink(topic)]
        by: H160,
    }

    /// Event emitted when contract is unpaused
    #[ink(event)]
    pub struct Unpaused {
        #[ink(topic)]
        by: H160,
    }

    /// Event emitted when an address is blacklisted
    #[ink(event)]
    pub struct Blacklisted {
        #[ink(topic)]
        account: H160,
    }

    /// Event emitted when an address is removed from blacklist
    #[ink(event)]
    pub struct RemovedFromBlacklist {
        #[ink(topic)]
        account: H160,
    }

    #[derive(Debug, PartialEq, Eq)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        InsufficientBalance,
        InsufficientAllowance,
        Paused,
        Blacklisted,
        Unauthorized,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct Token {
        /// Total token supply
        total_supply: Balance,
        /// Mapping from owner to balance
        balances: Mapping<H160, Balance>,
        /// Mapping from (owner, spender) to allowance
        allowances: Mapping<(H160, H160), Balance>,
        /// Contract owner
        owner: H160,
        /// Paused state
        paused: bool,
        /// Blacklisted addresses
        blacklist: Mapping<H160, bool>,
    }

    impl Token {
        /// Constructor that initializes the token with initial supply
        #[ink(constructor)]
        pub fn new(initial_supply: Balance) -> Self {
            let caller = Self::env().caller();
            let mut balances = Mapping::default();
            balances.insert(caller, &initial_supply);

            // Self::env().emit_event(Transfer {
            //     from: None,
            //     to: Some(caller),
            //     value: initial_supply,
            // });

            Self {
                total_supply: initial_supply,
                balances,
                allowances: Mapping::default(),
                owner: caller,
                paused: false,
                blacklist: Mapping::default(),
            }
        }

        /// Default constructor with 1,000,000 initial supply
        #[ink(constructor)]
        pub fn default() -> Self {
            Self::new(1000000)
        }

        /// Returns the total token supply
        #[ink(message)]
        pub fn total_supply(&self) -> Balance {
            self.total_supply
        }

        /// Returns the balance of the given account
        #[ink(message)]
        pub fn balance_of(&self, owner: H160) -> Balance {
            self.balances.get(owner).unwrap_or(0)
        }

        /// Returns the allowance for a spender approved by an owner
        #[ink(message)]
        pub fn allowance(&self, owner: H160, spender: H160) -> Balance {
            self.allowances.get((owner, spender)).unwrap_or(0)
        }

        /// Transfers tokens from the caller to another account
        #[ink(message)]
        pub fn transfer(&mut self, to: H160, value: Balance) -> Result<()> {
            let from = self.env().caller();
            self.transfer_from_to(&from, &to, value)?;
            Ok(())
        }

        /// Approves a spender to spend tokens on behalf of the caller
        #[ink(message)]
        pub fn approve(&mut self, spender: H160, value: Balance) -> Result<()> {
            let owner = self.env().caller();
            self.allowances.insert((owner, spender), &value);

            // self.env().emit_event(Approval {
            //     owner,
            //     spender,
            //     value,
            // });

            Ok(())
        }

        /// Transfers tokens from one account to another using allowance
        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: H160,
            to: H160,
            value: Balance,
        ) -> Result<()> {
            let caller = self.env().caller();
            let allowance = self.allowance(from, caller);

            if allowance < value {
                return Err(Error::InsufficientAllowance);
            }

            self.transfer_from_to(&from, &to, value)?;
            self.allowances.insert((from, caller), &allowance.saturating_sub(value));

            Ok(())
        }

        /// Mints new tokens to the caller's balance
        #[ink(message)]
        pub fn mint(&mut self, value: Balance) -> Result<()> {
            let caller = self.env().caller();
            let balance = self.balance_of(caller);

            self.balances.insert(caller, &balance.saturating_add(value));
            self.total_supply = self.total_supply.saturating_add(value);

            // self.env().emit_event(Transfer {
            //     from: None,
            //     to: Some(caller),
            //     value,
            // });

            Ok(())
        }

        /// Burns tokens from the caller's balance
        #[ink(message)]
        pub fn burn(&mut self, value: Balance) -> Result<()> {
            let caller = self.env().caller();
            let balance = self.balance_of(caller);

            if balance < value {
                return Err(Error::InsufficientBalance);
            }

            self.balances.insert(caller, &balance.saturating_sub(value));
            self.total_supply = self.total_supply.saturating_sub(value);

            // self.env().emit_event(Burn {
            //     from: caller,
            //     value,
            // });

            // self.env().emit_event(Transfer {
            //     from: Some(caller),
            //     to: None,
            //     value,
            // });

            Ok(())
        }

        /// Increases allowance for a spender
        #[ink(message)]
        pub fn increase_allowance(&mut self, spender: H160, delta_value: Balance) -> Result<()> {
            let owner = self.env().caller();
            let current_allowance = self.allowance(owner, spender);
            self.allowances.insert((owner, spender), &current_allowance.saturating_add(delta_value));
            Ok(())
        }

        /// Decreases allowance for a spender
        #[ink(message)]
        pub fn decrease_allowance(&mut self, spender: H160, delta_value: Balance) -> Result<()> {
            let owner = self.env().caller();
            let current_allowance = self.allowance(owner, spender);

            if current_allowance < delta_value {
                return Err(Error::InsufficientAllowance);
            }

            self.allowances.insert((owner, spender), &current_allowance.saturating_sub(delta_value));
            Ok(())
        }

        /// Pauses the contract (only owner)
        #[ink(message)]
        pub fn pause(&mut self) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(Error::Unauthorized);
            }

            self.paused = true;

            // self.env().emit_event(Paused { by: caller });

            Ok(())
        }

        /// Unpauses the contract (only owner)
        #[ink(message)]
        pub fn unpause(&mut self) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(Error::Unauthorized);
            }

            self.paused = false;

            // self.env().emit_event(Unpaused { by: caller });

            Ok(())
        }

        /// Returns whether the contract is paused
        #[ink(message)]
        pub fn is_paused(&self) -> bool {
            self.paused
        }

        /// Adds an address to the blacklist (only owner)
        #[ink(message)]
        pub fn blacklist_address(&mut self, account: H160) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(Error::Unauthorized);
            }

            self.blacklist.insert(account, &true);

            // self.env().emit_event(Blacklisted { account });

            Ok(())
        }

        /// Removes an address from the blacklist (only owner)
        #[ink(message)]
        pub fn remove_from_blacklist(&mut self, account: H160) -> Result<()> {
            let caller = self.env().caller();
            if caller != self.owner {
                return Err(Error::Unauthorized);
            }

            self.blacklist.remove(account);

            // self.env().emit_event(RemovedFromBlacklist { account });

            Ok(())
        }

        /// Checks if an address is blacklisted
        #[ink(message)]
        pub fn is_blacklisted(&self, account: H160) -> bool {
            self.blacklist.get(account).unwrap_or(false)
        }

        /// Batch transfer to multiple recipients
        #[ink(message)]
        pub fn batch_transfer(&mut self, recipients: Vec<(H160, Balance)>) -> Result<()> {
            for (to, value) in recipients {
                self.transfer(to, value)?;
            }
            Ok(())
        }

        /// Returns the contract owner
        #[ink(message)]
        pub fn owner(&self) -> H160 {
            self.owner
        }

        /// Internal transfer function with checks
        fn transfer_from_to(
            &mut self,
            from: &H160,
            to: &H160,
            value: Balance,
        ) -> Result<()> {
            // Check if contract is paused
            if self.paused {
                return Err(Error::Paused);
            }

            // Check if sender or recipient is blacklisted
            if self.is_blacklisted(*from) || self.is_blacklisted(*to) {
                return Err(Error::Blacklisted);
            }

            let from_balance = self.balance_of(*from);
            if from_balance < value {
                return Err(Error::InsufficientBalance);
            }

            self.balances.insert(from, &from_balance.saturating_sub(value));
            let to_balance = self.balance_of(*to);
            self.balances.insert(to, &to_balance.saturating_add(value));

            // self.env().emit_event(Transfer {
            //     from: Some(*from),
            //     to: Some(*to),
            //     value,
            // });

            Ok(())
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

        fn get_charlie() -> H160 {
            H160::from([3u8; 20])
        }

        #[ink::test]
        fn new_works() {
            let token = Token::new(1000);
            assert_eq!(token.total_supply(), 1000);
        }

        #[ink::test]
        fn balance_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let token = Token::new(1000);
            let bob = get_bob();

            assert_eq!(token.balance_of(accounts.alice), 1000);
            assert_eq!(token.balance_of(bob), 0);
        }

        #[ink::test]
        fn transfer_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert_eq!(token.balance_of(accounts.alice), 1000);
            assert_eq!(token.balance_of(bob), 0);

            assert!(token.transfer(bob, 100).is_ok());

            assert_eq!(token.balance_of(accounts.alice), 900);
            assert_eq!(token.balance_of(bob), 100);
        }

        #[ink::test]
        fn transfer_insufficient_balance_fails() {
            let mut token = Token::new(100);
            let bob = get_bob();

            let result = token.transfer(bob, 200);
            assert_eq!(result, Err(Error::InsufficientBalance));
        }

        #[ink::test]
        fn approve_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert_eq!(token.allowance(accounts.alice, bob), 0);
            assert!(token.approve(bob, 100).is_ok());
            assert_eq!(token.allowance(accounts.alice, bob), 100);
        }

        #[ink::test]
        fn transfer_from_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();
            let charlie = get_charlie();

            // Approve Bob to spend tokens
            assert!(token.approve(bob, 100).is_ok());

            // Set caller to Bob for transfer_from
            test::set_caller(bob);

            // Bob transfers from alice to Charlie
            assert!(token.transfer_from(accounts.alice, charlie, 50).is_ok());

            // Check balances
            assert_eq!(token.balance_of(accounts.alice), 950);
            assert_eq!(token.balance_of(charlie), 50);
            assert_eq!(token.allowance(accounts.alice, bob), 50);
        }

        #[ink::test]
        fn burn_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);

            assert!(token.burn(100).is_ok());
            assert_eq!(token.balance_of(accounts.alice), 900);
            assert_eq!(token.total_supply(), 900);
        }

        #[ink::test]
        fn pause_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert!(!token.is_paused());
            assert!(token.pause().is_ok());
            assert!(token.is_paused());

            let result = token.transfer(bob, 100);
            assert_eq!(result, Err(Error::Paused));
        }

        #[ink::test]
        fn blacklist_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert!(!token.is_blacklisted(bob));
            assert!(token.blacklist_address(bob).is_ok());
            assert!(token.is_blacklisted(bob));

            let result = token.transfer(bob, 100);
            assert_eq!(result, Err(Error::Blacklisted));
        }

        #[ink::test]
        fn batch_transfer_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();
            let charlie = get_charlie();

            let recipients = vec![
                (bob, 100),
                (charlie, 200),
            ];

            assert!(token.batch_transfer(recipients).is_ok());
            assert_eq!(token.balance_of(accounts.alice), 700);
            assert_eq!(token.balance_of(bob), 100);
            assert_eq!(token.balance_of(charlie), 200);
        }

        #[ink::test]
        fn only_owner_can_pause() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            test::set_caller(bob);
            let result = token.pause();
            assert_eq!(result, Err(Error::Unauthorized));
        }

        #[ink::test]
        fn only_owner_can_blacklist() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();
            let charlie = get_charlie();

            test::set_caller(bob);
            let result = token.blacklist_address(charlie);
            assert_eq!(result, Err(Error::Unauthorized));
        }

        #[ink::test]
        fn mint_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);

            assert_eq!(token.total_supply(), 1000);
            assert_eq!(token.balance_of(accounts.alice), 1000);

            assert!(token.mint(500).is_ok());

            assert_eq!(token.total_supply(), 1500);
            assert_eq!(token.balance_of(accounts.alice), 1500);
        }

        #[ink::test]
        fn increase_allowance_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert!(token.approve(bob, 100).is_ok());
            assert_eq!(token.allowance(accounts.alice, bob), 100);

            assert!(token.increase_allowance(bob, 50).is_ok());
            assert_eq!(token.allowance(accounts.alice, bob), 150);
        }

        #[ink::test]
        fn decrease_allowance_works() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert!(token.approve(bob, 100).is_ok());
            assert_eq!(token.allowance(accounts.alice, bob), 100);

            assert!(token.decrease_allowance(bob, 30).is_ok());
            assert_eq!(token.allowance(accounts.alice, bob), 70);
        }

        #[ink::test]
        fn decrease_allowance_insufficient_fails() {
            let accounts = get_default_accounts();
            test::set_caller(accounts.alice);

            let mut token = Token::new(1000);
            let bob = get_bob();

            assert!(token.approve(bob, 50).is_ok());

            let result = token.decrease_allowance(bob, 100);
            assert_eq!(result, Err(Error::InsufficientAllowance));
        }
    }
}
