#![no_std]
extern crate alloc;

use gmeta::{InOut, Metadata};
use gstd::{collections::BTreeMap, msg, prelude::*, ActorId, exec};
use scale_info::TypeInfo;

const ESCROW_FEE: u128 = 1_000_000_000_000; // 1 VARA in smallest units

#[derive(Encode, Decode, TypeInfo, Clone, Copy, Debug, PartialEq)]
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
}

#[derive(Encode, Decode, TypeInfo)]
pub struct SafeDeal {
    escrows: BTreeMap<u64, Escrow>,
    next_escrow_id: u64,
}

impl SafeDeal {
    pub fn new() -> Self {
        Self {
            escrows: BTreeMap::new(),
            next_escrow_id: 1,
        }
    }

    pub fn create_escrow(
        &mut self,
        seller: ActorId,
        timeout_blocks: u32,
    ) -> Result<u64, SafeDealError> {
        if seller == ActorId::zero() {
            return Err(SafeDealError::SellerCannotBeZero);
        }

        let buyer = msg::source();
        let total_received = msg::value();

        if total_received < ESCROW_FEE {
            return Err(SafeDealError::InsufficientFunds);
        }

        let amount = total_received - ESCROW_FEE;
        let current_block = exec::block_height();
        let timeout_at = current_block.saturating_add(timeout_blocks);
        let escrow_id = self.next_escrow_id;
        self.next_escrow_id = self.next_escrow_id.saturating_add(1);

        let escrow = Escrow {
            escrow_id,
            buyer,
            seller,
            amount,
            timeout_at,
            status: EscrowStatus::Pending,
            created_at: current_block,
        };

        self.escrows.insert(escrow_id, escrow);

        Ok(escrow_id)
    }

    pub fn confirm_delivery(&mut self, escrow_id: u64) -> Result<(), SafeDealError> {
        let caller = msg::source();
        let escrow = self
            .escrows
            .get_mut(&escrow_id)
            .ok_or(SafeDealError::InvalidEscrowId)?;

        if escrow.buyer != caller {
            return Err(SafeDealError::UnauthorizedActor);
        }

        if escrow.status != EscrowStatus::Pending {
            return Err(SafeDealError::EscrowAlreadyCompleted);
        }

        let seller = escrow.seller;
        let amount = escrow.amount;
        escrow.status = EscrowStatus::Completed;

        send_value(seller, amount)?;

        Ok(())
    }

    pub fn refund_on_timeout(&mut self, escrow_id: u64) -> Result<(), SafeDealError> {
        let escrow = self
            .escrows
            .get_mut(&escrow_id)
            .ok_or(SafeDealError::InvalidEscrowId)?;

        if escrow.status != EscrowStatus::Pending {
            return Err(SafeDealError::EscrowAlreadyCompleted);
        }

        let current_block = exec::block_height();
        if current_block < escrow.timeout_at {
            return Err(SafeDealError::TimeoutNotReached);
        }

        let buyer = escrow.buyer;
        let amount = escrow.amount;
        escrow.status = EscrowStatus::Refunded;

        send_value(buyer, amount)?;

        Ok(())
    }

    pub fn get_escrow(&self, escrow_id: u64) -> Option<Escrow> {
        self.escrows.get(&escrow_id).cloned()
    }

    pub fn get_escrows_by_buyer(&self, buyer: ActorId) -> Vec<Escrow> {
        self.escrows
            .values()
            .filter(|escrow| escrow.buyer == buyer)
            .cloned()
            .collect()
    }

    pub fn get_escrows_by_seller(&self, seller: ActorId) -> Vec<Escrow> {
        self.escrows
            .values()
            .filter(|escrow| escrow.seller == seller)
            .cloned()
            .collect()
    }

    pub fn get_fee() -> u128 {
        ESCROW_FEE
    }
}

fn send_value(to: ActorId, amount: u128) -> Result<(), SafeDealError> {
    if amount == 0 {
        return Ok(());
    }

    let _ = msg::send_with_gas(to, "", exec::gas_available() / 2, amount)
        .map_err(|_| SafeDealError::TransferFailed)?;

    Ok(())
}

static mut SAFE_DEAL: Option<SafeDeal> = None;

#[derive(Encode, Decode, TypeInfo)]
pub enum Command {
    CreateEscrow {
        seller: ActorId,
        timeout_blocks: u32,
    },
    ConfirmDelivery {
        escrow_id: u64,
    },
    RefundOnTimeout {
        escrow_id: u64,
    },
    GetEscrow {
        escrow_id: u64,
    },
    GetEscrowsByBuyer {
        buyer: ActorId,
    },
    GetEscrowsBySeller {
        seller: ActorId,
    },
    GetFee,
}

#[derive(Encode, Decode, TypeInfo)]
pub enum Reply {
    EscrowCreated(u64),
    DeliveryConfirmed,
    RefundProcessed,
    EscrowInfo(Option<Escrow>),
    EscrowList(Vec<Escrow>),
    Fee(u128),
    Error(SafeDealError),
}

pub struct SafeDealMetadata;

impl Metadata for SafeDealMetadata {
    type Init = ();
    type Handle = InOut<Command, Reply>;
    type Others = ();
    type Reply = ();
    type Signal = ();
    type State = ();
}

#[gstd::async_main]
async fn main() {
    let command: Command = msg::load().expect("Failed to load command");

    let safe_deal = unsafe { SAFE_DEAL.get_or_insert_with(SafeDeal::new) };

    match command {
        Command::CreateEscrow {
            seller,
            timeout_blocks,
        } => {
            match safe_deal.create_escrow(seller, timeout_blocks) {
                Ok(escrow_id) => {
                    let _ = msg::reply(Reply::EscrowCreated(escrow_id), 0);
                }
                Err(e) => {
                    let _ = msg::reply(Reply::Error(e), 0);
                }
            }
        }
        Command::ConfirmDelivery { escrow_id } => {
            match safe_deal.confirm_delivery(escrow_id) {
                Ok(_) => {
                    let _ = msg::reply(Reply::DeliveryConfirmed, 0);
                }
                Err(e) => {
                    let _ = msg::reply(Reply::Error(e), 0);
                }
            }
        }
        Command::RefundOnTimeout { escrow_id } => {
            match safe_deal.refund_on_timeout(escrow_id) {
                Ok(_) => {
                    let _ = msg::reply(Reply::RefundProcessed, 0);
                }
                Err(e) => {
                    let _ = msg::reply(Reply::Error(e), 0);
                }
            }
        }
        Command::GetEscrow { escrow_id } => {
            let escrow = safe_deal.get_escrow(escrow_id);
            let _ = msg::reply(Reply::EscrowInfo(escrow), 0);
        }
        Command::GetEscrowsByBuyer { buyer } => {
            let escrows = safe_deal.get_escrows_by_buyer(buyer);
            let _ = msg::reply(Reply::EscrowList(escrows), 0);
        }
        Command::GetEscrowsBySeller { seller } => {
            let escrows = safe_deal.get_escrows_by_seller(seller);
            let _ = msg::reply(Reply::EscrowList(escrows), 0);
        }
        Command::GetFee => {
            let fee = SafeDeal::get_fee();
            let _ = msg::reply(Reply::Fee(fee), 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_safe_deal() {
        let safe_deal = SafeDeal::new();
        assert_eq!(safe_deal.next_escrow_id, 1);
        assert!(safe_deal.escrows.is_empty());
    }

    #[test]
    fn test_fee_constant() {
        assert_eq!(SafeDeal::get_fee(), ESCROW_FEE);
    }
}
