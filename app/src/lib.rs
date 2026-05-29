#![no_std]
extern crate alloc;

use sails_rs::prelude::*;
use alloc::collections::BTreeMap;

const ESCROW_FEE: u128 = 1_000_000_000_000; // 1 VARA in smallest units

#[derive(Encode, Decode, TypeInfo, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscrowStatus {
    Pending,
    Completed,
    Refunded,
}

#[derive(Encode, Decode, TypeInfo, Clone, Debug)]
pub struct Escrow {
    pub escrow_id: u64,
    pub buyer: ActorId,
    pub seller: ActorId,
    pub amount: u128,
    pub timeout_at: u32,
    pub status: EscrowStatus,
    pub created_at: u32,
}

#[derive(Encode, Decode, TypeInfo)]
pub enum SafeDealError {
    InsufficientFunds,
    InvalidEscrowId,
    InvalidStatus,
    UnauthorizedActor,
    EscrowAlreadyCompleted,
    TimeoutNotReached,
    SellerCannotBeZero,
    TransferFailed,
    SelfLoop,
}

#[derive(Encode, Decode, TypeInfo)]
pub struct SafeDealState {
    escrows: BTreeMap<u64, Escrow>,
    next_escrow_id: u64,
}

#[sails_rs::program]
pub struct Program {
    state: RefCell<SafeDealState>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(SafeDealState {
                escrows: BTreeMap::new(),
                next_escrow_id: 1,
            }),
        }
    }

    pub fn safedeal(&self) -> SafeDealService {
        SafeDealService::new(&self.state)
    }
}

#[derive(Encode, Decode, TypeInfo)]
pub enum SafeDealEvent {
    EscrowCreated {
        id: u64,
        buyer: ActorId,
        seller: ActorId,
        amount: u128,
        deadline: u32,
    },
    DeliveryConfirmed {
        id: u64,
        seller: ActorId,
        amount: u128,
    },
    EscrowRefunded {
        id: u64,
        buyer: ActorId,
        amount: u128,
    },
}

pub struct SafeDealService<'a> {
    state: &'a RefCell<SafeDealState>,
}

impl<'a> SafeDealService<'a> {
    pub fn new(state: &'a RefCell<SafeDealState>) -> Self {
        Self { state }
    }
}

#[sails_rs::service(events = SafeDealEvent)]
impl<'a> SafeDealService<'a> {
    #[export]
    pub fn create_escrow(
        &mut self,
        seller: ActorId,
        timeout_blocks: u32,
    ) -> Result<u64, SafeDealError> {
        if Syscall::message_source() == Syscall::program_id() {
            return Err(SafeDealError::SelfLoop);
        }

        let total_received = Syscall::message_value();
        if total_received < ESCROW_FEE {
            return Err(SafeDealError::InsufficientFunds);
        }

        if seller == ActorId::zero() {
            return Err(SafeDealError::SellerCannotBeZero);
        }

        let buyer = Syscall::message_source();
        let amount = total_received - ESCROW_FEE;
        let current_block = Syscall::block_height();
        let timeout_at = current_block.saturating_add(timeout_blocks);

        let escrow_id = {
            let mut state = self.state.borrow_mut();
            let id = state.next_escrow_id;
            state.next_escrow_id = state.next_escrow_id.saturating_add(1);

            state.escrows.insert(
                id,
                Escrow {
                    escrow_id: id,
                    buyer,
                    seller,
                    amount,
                    timeout_at,
                    status: EscrowStatus::Pending,
                    created_at: current_block,
                },
            );
            id
        };

        self.emit_event(SafeDealEvent::EscrowCreated {
            id: escrow_id,
            buyer,
            seller,
            amount,
            deadline: timeout_at,
        });

        Ok(escrow_id)
    }

    #[export]
    pub fn confirm_delivery(&mut self, escrow_id: u64) -> Result<(), SafeDealError> {
        let caller = Syscall::message_source();

        let (seller, amount) = {
            let mut state = self.state.borrow_mut();
            let escrow = state
                .escrows
                .get_mut(&escrow_id)
                .ok_or(SafeDealError::InvalidEscrowId)?;

            if escrow.buyer != caller {
                return Err(SafeDealError::UnauthorizedActor);
            }

            if escrow.status != EscrowStatus::Pending {
                return Err(SafeDealError::EscrowAlreadyCompleted);
            }

            escrow.status = EscrowStatus::Completed;
            (escrow.seller, escrow.amount)
        };

        Syscall::send_value(seller, amount)
            .map_err(|_| SafeDealError::TransferFailed)?;

        self.emit_event(SafeDealEvent::DeliveryConfirmed {
            id: escrow_id,
            seller,
            amount,
        });

        Ok(())
    }

    #[export]
    pub fn refund_on_timeout(&mut self, escrow_id: u64) -> Result<(), SafeDealError> {
        let (buyer, amount) = {
            let mut state = self.state.borrow_mut();
            let escrow = state
                .escrows
                .get_mut(&escrow_id)
                .ok_or(SafeDealError::InvalidEscrowId)?;

            if escrow.status != EscrowStatus::Pending {
                return Err(SafeDealError::EscrowAlreadyCompleted);
            }

            let current_block = Syscall::block_height();
            if current_block < escrow.timeout_at {
                return Err(SafeDealError::TimeoutNotReached);
            }

            escrow.status = EscrowStatus::Refunded;
            (escrow.buyer, escrow.amount)
        };

        Syscall::send_value(buyer, amount)
            .map_err(|_| SafeDealError::TransferFailed)?;

        self.emit_event(SafeDealEvent::EscrowRefunded {
            id: escrow_id,
            buyer,
            amount,
        });

        Ok(())
    }

    #[export]
    pub fn get_escrow(&self, escrow_id: u64) -> Option<Escrow> {
        self.state.borrow().escrows.get(&escrow_id).cloned()
    }

    #[export]
    pub fn get_escrows_by_buyer(&self, buyer: ActorId) -> Vec<Escrow> {
        self.state
            .borrow()
            .escrows
            .values()
            .filter(|escrow| escrow.buyer == buyer)
            .cloned()
            .collect()
    }

    #[export]
    pub fn get_escrows_by_seller(&self, seller: ActorId) -> Vec<Escrow> {
        self.state
            .borrow()
            .escrows
            .values()
            .filter(|escrow| escrow.seller == seller)
            .cloned()
            .collect()
    }

    #[export]
    pub fn get_fee() -> u128 {
        ESCROW_FEE
    }
}
