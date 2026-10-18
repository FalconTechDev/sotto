import { useEffect, useState } from "react";

import {
  acceptPayerTransfer,
  createPayerTransferPaymentSetup,
  fetchPayerTransfer,
  fetchPayerTransferCapabilities,
  fetchPayerTransfers,
  me,
  withdrawPayerTransfer,
  type PayerTransfer,
} from "./api";

function message(error: unknown): string {
  if (!(error instanceof Error)) return "The transfer could not be updated. Refresh and try again.";
  const status = /server error \((\d+)\)/.exec(error.message)?.[1];
  if (status === "403") return "Your authority for this transfer has changed. Ask the current payer to start a new request.";
  if (status === "404") return "This transfer is no longer available. Refresh the list for its latest state.";
  if (status === "409") return "The transfer terms have changed or the invitation has expired. Request a fresh transfer.";
  return "The transfer service is temporarily unavailable. Refresh and try again.";
}

function date(epoch: number): string {
  return new Date(epoch * 1000).toLocaleDateString("en-GB");
}

function pounds(pence: number): string {
  return new Intl.NumberFormat("en-GB", { style: "currency", currency: "GBP" }).format(pence / 100);
}

function payer(kind: string, organizationId: string | null): string {
  return kind === "personal" ? "personal billing" : `organisation ${organizationId ?? "billing"}`;
}

function stateCopy(state: string): string {
  switch (state) {
    case "awaiting_consent": return "Waiting for the other payer to respond.";
    case "pending": return "Accepted. The billing change is being prepared; it is not complete yet.";
    case "destination_prepared": return "The change is being prepared for the renewal date.";
    case "destination_paid":
    case "source_adjustment_pending": return "Payment is recorded and the payer handover is being verified.";
    case "source_restoration_pending": return "The previous payer is being restored after a payment issue.";
    case "completed": return "Payer change completed after provider verification.";
    case "failed": return "This request ended without changing the payer.";
    case "expired": return "This invitation expired without changing the payer.";
    case "unknown": return "This transfer needs support review. It will not continue automatically.";
    default: return "Transfer status is being checked.";
  }
}

export function PayerTransferInbox() {
  const [available, setAvailable] = useState(false);
  const [setupAvailable, setSetupAvailable] = useState(false);
  const [userId, setUserId] = useState<string | null>(null);
  const [transfers, setTransfers] = useState<PayerTransfer[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const rows = await fetchPayerTransfers();
      setTransfers(rows);
    } catch (e) {
      setError(message(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    let current = true;
    void Promise.all([fetchPayerTransferCapabilities(), me()])
      .then(([capabilities, account]) => {
        if (!current) return;
        setAvailable(capabilities.available);
        setSetupAvailable(capabilities.setupAvailable);
        setUserId(account?.userId ?? null);
        if (capabilities.available) void refresh();
        else setLoading(false);
      })
      .catch(() => {
        // Older servers and temporary discovery failures keep the existing account page usable.
        if (current) setLoading(false);
      });
    return () => { current = false; };
  }, []);

  useEffect(() => {
    const onTransferUpdated = () => { if (available) void refresh(); };
    window.addEventListener("sotto:payer-transfer-updated", onTransferUpdated);
    return () => window.removeEventListener("sotto:payer-transfer-updated", onTransferUpdated);
  }, [available]);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const returnId = params.get("transfer_id");
    const outcome = params.get("billing");
    if (outcome !== "transfer_setup" && outcome !== "transfer_setup_cancelled") return;
    if (returnId !== null) {
      setNotice(outcome === "transfer_setup"
        ? "Payment method setup returned. Refreshing the transfer; provider confirmation is still required."
        : "Payment method setup was cancelled. The transfer has not completed.");
      if (available) {
        void fetchPayerTransfer(returnId)
          .then((transfer) => setTransfers((current) => [
            transfer,
            ...current.filter((item) => item.transferId !== transfer.transferId),
          ]))
          .catch(() => {})
          .finally(() => void refresh());
      }
    }
    params.delete("billing");
    params.delete("transfer_id");
    const query = params.toString();
    window.history.replaceState(null, "", window.location.pathname + (query ? `?${query}` : "") + window.location.hash);
  }, [available]);

  async function act(transfer: PayerTransfer, action: "accept" | "withdraw" | "setup") {
    setBusyId(transfer.transferId);
    setError(null);
    setNotice(null);
    try {
      if (action === "accept") await acceptPayerTransfer(transfer.transferId);
      else if (action === "withdraw") await withdrawPayerTransfer(transfer.transferId);
      else {
        const setup = await createPayerTransferPaymentSetup(transfer.transferId);
        if (setup.checkoutUrl !== null) {
          window.location.assign(setup.checkoutUrl);
          return;
        }
        setNotice("Payment method setup is pending. Refresh the transfer shortly.");
      }
      await refresh();
    } catch (e) {
      setError(message(e));
      await refresh();
    } finally {
      setBusyId(null);
    }
  }

  if (!available) return null;

  return <section aria-labelledby="transfer-inbox-heading">
    <h2 id="transfer-inbox-heading">Payer transfers</h2>
    <p className="muted">Transfers take effect at renewal. The current payer remains responsible through the paid term; no mid-term credit is issued.</p>
    {error !== null && <p role="alert">{error}</p>}
    {notice !== null && <p className="notice" role="status">{notice}</p>}
    <button disabled={loading || busyId !== null} onClick={() => void refresh()}>{loading ? "Refreshing transfers…" : "Refresh transfers"}</button>
    {loading && <p className="muted" role="status">Loading transfer requests…</p>}
    {!loading && transfers.length === 0 && <p className="muted">There are no payer transfer requests for this account.</p>}
    <ul className="items">
      {transfers.map((transfer) => {
        const incoming = transfer.participantRole === "counterparty";
        const actionable = transfer.state === "awaiting_consent";
        const setupRequired = transfer.state === "pending"
          && transfer.destinationKind === "personal"
          && transfer.beneficiaryId === userId
          && transfer.personalPaymentSetupNeeded
          && setupAvailable;
        return <li key={transfer.transferId}>
          <strong>{incoming ? "Incoming request" : "Outgoing request"}</strong>
          <span className="meta">{payer(transfer.sourceKind, transfer.sourceOrganizationId)} → {payer(transfer.destinationKind, transfer.destinationOrganizationId)}</span>
          <p>{pounds(transfer.amountPence)} per {transfer.interval}; renewal {date(transfer.effectiveFrom)}.</p>
          {actionable && <p className="muted">Respond by {date(transfer.consentExpiresAtEpoch)}.</p>}
          <p className="muted">{stateCopy(transfer.state)}</p>
          {transfer.state === "unknown" && <p className="muted">Contact Sotto support before making another billing change.</p>}
          {actionable && incoming && transfer.destinationKind === "personal" && !setupAvailable
            && <p className="muted">Personal payment setup is not available on this server yet. The request has not been accepted.</p>}
          {actionable && incoming && <>
            {(transfer.destinationKind !== "personal" || setupAvailable) && <button disabled={busyId !== null} onClick={() => void act(transfer, "accept")}>{busyId === transfer.transferId ? "Accepting…" : "Accept transfer request"}</button>}{" "}
            <button disabled={busyId !== null} onClick={() => void act(transfer, "withdraw")}>Decline request</button>
          </>}
          {actionable && !incoming && <button disabled={busyId !== null} onClick={() => void act(transfer, "withdraw")}>Withdraw request</button>}
          {setupRequired && <button disabled={busyId !== null} onClick={() => void act(transfer, "setup")}>Set up payment method</button>}
        </li>;
      })}
    </ul>
  </section>;
}
