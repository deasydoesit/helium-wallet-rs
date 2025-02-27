use crate::{
    cmd::*,
    keypair::PublicKey,
    result::Result,
    traits::{TxnEnvelope, TxnFee, TxnSign, B64},
};
use helium_api::accounts;
use serde_json::json;

#[derive(Debug, StructOpt)]
/// Burn HNT to Data Credits (DC) from this wallet to given payees wallet.
pub struct Cmd {
    /// Account address to send the resulting DC to.
    #[structopt(long)]
    payee: PublicKey,

    /// Memo field to include. Provide as a base64 encoded string
    #[structopt(long)]
    memo: Option<String>,

    /// Amount of HNT to burn to DC
    #[structopt(long)]
    amount: Hnt,

    /// Commit the payment to the API
    #[structopt(long)]
    commit: bool,
}

impl Cmd {
    pub async fn run(&self, opts: Opts) -> Result {
        let password = get_password(false)?;
        let wallet = load_wallet(opts.files)?;

        let client = Client::new_with_base_url(api_url(wallet.public_key.network));

        let keypair = wallet.decrypt(password.as_bytes())?;
        let account = accounts::get(&client, &keypair.public_key().to_string()).await?;
        let memo = match &self.memo {
            None => 0,
            Some(s) => u64::from_b64(&s)?,
        };

        let mut txn = BlockchainTxnTokenBurnV1 {
            fee: 0,
            payee: self.payee.to_bytes().to_vec(),
            amount: u64::from(self.amount),
            payer: keypair.public_key().into(),
            memo,
            nonce: account.speculative_nonce + 1,
            signature: Vec::new(),
        };
        txn.fee = txn.txn_fee(&get_txn_fees(&client).await?)?;
        txn.signature = txn.sign(&keypair)?;

        let envelope = txn.in_envelope();
        let status = maybe_submit_txn(self.commit, &client, &envelope).await?;
        print_txn(&txn, &envelope, &status, opts.format)
    }
}

fn print_txn(
    txn: &BlockchainTxnTokenBurnV1,
    envelope: &BlockchainTxn,
    status: &Option<PendingTxnStatus>,
    format: OutputFormat,
) -> Result {
    match format {
        OutputFormat::Table => {
            ptable!(
                ["Key", "Value"],
                ["Payee", PublicKey::from_bytes(&txn.payee)?.to_string()],
                ["Memo", txn.memo.to_b64()?],
                ["Amount", Hnt::from(txn.amount)],
                ["Fee", txn.fee],
                ["Nonce", txn.nonce],
                ["Hash", status_str(status)]
            );
            print_footer(status)
        }
        OutputFormat::Json => {
            let table = json!({
                "payee": PublicKey::from_bytes(&txn.payee)?.to_string(),
                "amount": Hnt::from(txn.amount),
                "memo": txn.memo.to_b64()?,
                "fee": txn.fee,
                "nonce": txn.nonce,
                "hash": status_json(status),
                "txn": envelope.to_b64()?
            });
            print_json(&table)
        }
    }
}
