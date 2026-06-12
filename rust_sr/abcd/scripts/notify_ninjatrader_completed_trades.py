#!/usr/bin/env python3
"""
Email completed NinjaTrader trade results once.

This module is called by the live monitor right after a trade is marked
completed. It uses email_notified_at as the duplicate guard, so each completed
signal can only send one completion email. SMTP credentials are read from
environment variables so secrets stay out of the repo.
"""

from __future__ import annotations

import argparse
import json
import os
import smtplib
import sys
from email.message import EmailMessage
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import ai_wave_rider_research as wave


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--signal-uid", default="")
    parser.add_argument("--account", default="DEMO5859105")
    parser.add_argument("--instrument", default="", help="Optional instrument filter, e.g. HO JUL26.")
    parser.add_argument("--dry-run", action="store_true", help="Print emails instead of sending them.")
    parser.add_argument("--mark-dry-run", action="store_true", help="Mark rows as notified even in dry-run mode.")
    return parser.parse_args()


def env_bool(name: str, default: bool = True) -> bool:
    value = os.environ.get(name)
    if value is None:
        return default
    return str(value).strip().lower() not in {"0", "false", "no", "off"}


def email_config() -> dict[str, Any]:
    return {
        "host": os.environ.get("ABCD_SMTP_HOST", "").strip(),
        "port": int(os.environ.get("ABCD_SMTP_PORT", "587") or 587),
        "user": os.environ.get("ABCD_SMTP_USER", "").strip(),
        "password": os.environ.get("ABCD_SMTP_PASSWORD", ""),
        "sender": os.environ.get("ABCD_EMAIL_FROM", os.environ.get("ABCD_SMTP_USER", "")).strip(),
        "recipients": [
            item.strip()
            for item in os.environ.get("ABCD_EMAIL_TO", "").replace(";", ",").split(",")
            if item.strip()
        ],
        "tls": env_bool("ABCD_SMTP_TLS", True),
    }


def config_missing(config: dict[str, Any]) -> list[str]:
    missing = []
    if not config["host"]:
        missing.append("ABCD_SMTP_HOST")
    if not config["sender"]:
        missing.append("ABCD_EMAIL_FROM or ABCD_SMTP_USER")
    if not config["recipients"]:
        missing.append("ABCD_EMAIL_TO")
    return missing


def ensure_columns(conn) -> None:
    columns = {
        "email_notified_at": "DATETIME(6) NULL",
        "email_notify_status": "VARCHAR(32) NULL",
        "email_notify_error": "TEXT NULL",
        "email_notify_attempts": "BIGINT NOT NULL DEFAULT 0",
    }
    with conn.cursor() as cur:
        cur.execute(
            """
            SELECT COLUMN_NAME
            FROM information_schema.columns
            WHERE table_schema = DATABASE()
              AND table_name = 'ninjatrader_order_signals'
            """
        )
        existing = {str(row["COLUMN_NAME"]) for row in cur.fetchall()}
        for name, definition in columns.items():
            if name not in existing:
                cur.execute(f"ALTER TABLE ninjatrader_order_signals ADD COLUMN {name} {definition}")
    conn.commit()


def fetch_completed_rows(conn, args: argparse.Namespace) -> list[dict[str, Any]]:
    where = ["status = 'completed'", "email_notified_at IS NULL"]
    params: list[Any] = []
    signal_uid = str(getattr(args, "signal_uid", "") or "").strip()
    if signal_uid:
        where.append("signal_uid = %s")
        params.append(signal_uid)
    if args.account:
        where.append("account_name = %s")
        params.append(args.account)
    if args.instrument:
        where.append("instrument = %s")
        params.append(args.instrument.upper())
    updated_after = getattr(args, "updated_after", None)
    if updated_after is not None:
        where.append("updated_at >= %s")
        params.append(updated_after)
    params.append(max(1, int(args.limit)))
    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                id, signal_uid, account_name, instrument, root_symbol, side, quantity,
                expected_price, actual_trigger_price, stop_price, target_price, tick_size,
                accounting_instrument, accounting_root_symbol, accounting_tick_value,
                accounting_risk_dollars, execution_tick_value, execution_risk_dollars,
                exit_price, exit_action, exit_order_name, exit_received_at,
                realized_ticks, realized_execution_dollars, realized_accounting_dollars,
                status_message, expected_ai_run_id, expected_setup_id, expected_template_uid,
                notes, created_at, updated_at, claimed_at, triggered_at
            FROM ninjatrader_order_signals
            WHERE {' AND '.join(where)}
            ORDER BY updated_at ASC, id ASC
            LIMIT %s
            """,
            params,
        )
        return list(cur.fetchall())


def fmt_money(value: Any) -> str:
    if value is None:
        return "N/A"
    try:
        return f"${float(value):,.2f}"
    except (TypeError, ValueError):
        return "N/A"


def fmt_float(value: Any, places: int = 4) -> str:
    if value is None:
        return "N/A"
    try:
        return f"{float(value):,.{places}f}"
    except (TypeError, ValueError):
        return "N/A"


def fmt_ticks(value: Any) -> str:
    if value is None:
        return "N/A"
    try:
        return f"{float(value):,.1f}"
    except (TypeError, ValueError):
        return "N/A"


def fmt_signed_money(value: Any) -> str:
    if value is None:
        return "N/A"
    try:
        number = float(value)
    except (TypeError, ValueError):
        return "N/A"
    return f"-${abs(number):,.2f}" if number < 0 else f"+${number:,.2f}"


def fmt_pct(value: Any) -> str:
    if value is None:
        return "N/A"
    try:
        return f"{float(value):.1f}%"
    except (TypeError, ValueError):
        return "N/A"


def parsed_notes(row: dict[str, Any]) -> dict[str, Any]:
    text = row.get("notes")
    if not text:
        return {}
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def first_float(*values: Any) -> float | None:
    for value in values:
        if value is None:
            continue
        try:
            return float(value)
        except (TypeError, ValueError):
            continue
    return None


def fetch_run_summary(conn, row: dict[str, Any]) -> dict[str, Any]:
    run_id = row.get("expected_ai_run_id")
    account_name = row.get("account_name")
    where = ["status = 'completed'"]
    params: list[Any] = []
    if run_id:
        where.append("expected_ai_run_id = %s")
        params.append(run_id)
    else:
        where.append("expected_ai_run_id IS NULL")
    if account_name:
        where.append("account_name = %s")
        params.append(account_name)

    with conn.cursor() as cur:
        cur.execute(
            f"""
            SELECT
                signal_uid, realized_ticks, realized_accounting_dollars,
                realized_execution_dollars, accounting_instrument, instrument,
                notes, updated_at
            FROM ninjatrader_order_signals
            WHERE {' AND '.join(where)}
            ORDER BY updated_at ASC, id ASC
            """,
            params,
        )
        rows = list(cur.fetchall())

    trades = 0
    wins = 0
    losses = 0
    flat = 0
    total_accounting_pnl = 0.0
    total_execution_pnl = 0.0
    total_ticks = 0.0
    starting_cash = first_float(os.environ.get("ABCD_EMAIL_STARTING_CASH"))
    accounting_instrument = row.get("accounting_instrument") or row.get("instrument")

    for item in rows:
        notes = parsed_notes(item)
        if starting_cash is None:
            starting_cash = first_float(notes.get("sim_account_cash"))
        if not accounting_instrument:
            accounting_instrument = item.get("accounting_instrument") or item.get("instrument")

        pnl = first_float(item.get("realized_accounting_dollars"), item.get("realized_execution_dollars"))
        if pnl is None:
            continue
        trades += 1
        total_accounting_pnl += pnl
        total_execution_pnl += first_float(item.get("realized_execution_dollars")) or 0.0
        total_ticks += first_float(item.get("realized_ticks")) or 0.0
        if pnl > 0:
            wins += 1
        elif pnl < 0:
            losses += 1
        else:
            flat += 1

    current_balance = None if starting_cash is None else starting_cash + total_accounting_pnl
    return_pct = None
    if starting_cash not in (None, 0):
        return_pct = (total_accounting_pnl / starting_cash) * 100.0

    return {
        "run_id": run_id,
        "accounting_instrument": accounting_instrument,
        "starting_cash": starting_cash,
        "current_balance": current_balance,
        "return_pct": return_pct,
        "trades": trades,
        "wins": wins,
        "losses": losses,
        "flat": flat,
        "win_rate": (wins / trades * 100.0) if trades else None,
        "total_accounting_pnl": total_accounting_pnl if trades else None,
        "total_execution_pnl": total_execution_pnl if trades else None,
        "total_ticks": total_ticks if trades else None,
    }


def build_email(
    row: dict[str, Any],
    sender: str,
    recipients: list[str],
    run_summary: dict[str, Any] | None = None,
) -> EmailMessage:
    notes = parsed_notes(row)
    run_summary = run_summary or {}
    side = str(row.get("side") or "").upper()
    acct_instrument = row.get("accounting_instrument") or row.get("instrument")
    acct_pnl = row.get("realized_accounting_dollars")
    exec_pnl = row.get("realized_execution_dollars")
    signal_uid = row.get("signal_uid")
    subject_pnl = fmt_money(acct_pnl if acct_pnl is not None else exec_pnl)
    subject = f"ABCD trade completed: {row.get('instrument') or 'UNKNOWN'} {side} {subject_pnl}"

    body = "\n".join(
        [
            "ABCD NinjaTrader trade completed",
            "",
            f"Signal: {signal_uid}",
            f"Account: {row.get('account_name') or 'N/A'}",
            f"Execution instrument: {row.get('instrument') or 'N/A'}",
            f"Accounting instrument: {acct_instrument or 'N/A'}",
            f"Side / Qty: {side or 'N/A'} / {row.get('quantity') or 'N/A'}",
            "",
            f"Entry fill: {fmt_float(row.get('actual_trigger_price'))}",
            f"Exit fill: {fmt_float(row.get('exit_price'))}",
            f"Stop: {fmt_float(row.get('stop_price'))}",
            f"Exit action: {row.get('exit_action') or 'N/A'}",
            f"Exit order: {row.get('exit_order_name') or 'N/A'}",
            "",
            f"Realized ticks: {fmt_ticks(row.get('realized_ticks'))}",
            f"MHO-equivalent P/L: {fmt_money(acct_pnl)}",
            f"Full HO execution P/L: {fmt_money(exec_pnl)}",
            f"MHO-equivalent planned risk: {fmt_money(row.get('accounting_risk_dollars'))}",
            f"Full HO planned risk: {fmt_money(row.get('execution_risk_dollars'))}",
            "",
            "Run-to-date summary",
            f"Completed trades: {run_summary.get('trades', 'N/A')}",
            f"Wins / Losses / Flat: {run_summary.get('wins', 'N/A')} / {run_summary.get('losses', 'N/A')} / {run_summary.get('flat', 'N/A')}",
            f"Win rate: {fmt_pct(run_summary.get('win_rate'))}",
            f"Total {run_summary.get('accounting_instrument') or acct_instrument or 'accounting'} P/L: {fmt_signed_money(run_summary.get('total_accounting_pnl'))}",
            f"Starting tracked cash: {fmt_money(run_summary.get('starting_cash'))}",
            f"Current tracked balance: {fmt_money(run_summary.get('current_balance'))}",
            f"Tracked return: {fmt_pct(run_summary.get('return_pct'))}",
            f"Total full HO execution P/L: {fmt_signed_money(run_summary.get('total_execution_pnl'))}",
            "",
            f"Triggered at: {row.get('triggered_at') or 'N/A'}",
            f"Completed/updated at: {row.get('updated_at') or 'N/A'}",
            f"Status: {row.get('status_message') or 'N/A'}",
            "",
            f"Model run: {row.get('expected_ai_run_id') or 'N/A'}",
            f"Setup: {row.get('expected_setup_id') or 'N/A'}",
            f"Template: {row.get('expected_template_uid') or 'N/A'}",
            f"Stage 2 score: {notes.get('stage2_score', 'N/A')}",
        ]
    )

    message = EmailMessage()
    message["From"] = sender
    message["To"] = ", ".join(recipients)
    message["Subject"] = subject
    message.set_content(body)
    return message


def send_email(message: EmailMessage, config: dict[str, Any]) -> None:
    with smtplib.SMTP(config["host"], int(config["port"]), timeout=15) as server:
        if config["tls"]:
            server.starttls()
        if config["user"]:
            server.login(config["user"], config["password"])
        server.send_message(message)


def validate_smtp_login(config: dict[str, Any]) -> str | None:
    try:
        with smtplib.SMTP(config["host"], int(config["port"]), timeout=15) as server:
            if config["tls"]:
                server.starttls()
            if config["user"]:
                server.login(config["user"], config["password"])
        return None
    except Exception as exc:  # pragma: no cover - depends on external SMTP
        return str(exc)


def mark_sent(conn, row_id: int, status: str, error: str | None = None) -> None:
    with conn.cursor() as cur:
        cur.execute(
            """
            UPDATE ninjatrader_order_signals
            SET email_notified_at = CASE WHEN %s = 'sent' THEN CURRENT_TIMESTAMP(6) ELSE email_notified_at END,
                email_notify_status = %s,
                email_notify_error = %s,
                email_notify_attempts = email_notify_attempts + 1
            WHERE id = %s
            """,
            (status, status, error[:1000] if error else None, row_id),
        )
    conn.commit()


def process_once(conn, args: argparse.Namespace, config: dict[str, Any], dry_run: bool) -> int:
    rows = fetch_completed_rows(conn, args)
    if not rows:
        return 0
    for row in rows:
        run_summary = fetch_run_summary(conn, row)
        message = build_email(
            row,
            config["sender"] or "abcd@example.local",
            config["recipients"] or ["dry-run@example.local"],
            run_summary=run_summary,
        )
        try:
            if dry_run:
                print(
                    json.dumps(
                        {
                            "event": "email_dry_run",
                            "signal_uid": row.get("signal_uid"),
                            "subject": message["Subject"],
                            "body": message.get_content(),
                        },
                        default=str,
                    ),
                    flush=True,
                )
                if args.mark_dry_run:
                    mark_sent(conn, int(row["id"]), "dry_run")
            else:
                send_email(message, config)
                mark_sent(conn, int(row["id"]), "sent")
                print(json.dumps({"event": "email_sent", "signal_uid": row.get("signal_uid")}), flush=True)
        except Exception as exc:  # pragma: no cover - depends on external SMTP
            mark_sent(conn, int(row["id"]), "failed", str(exc))
            print(
                json.dumps({"event": "email_failed", "signal_uid": row.get("signal_uid"), "error": str(exc)}),
                flush=True,
            )
    return len(rows)


def notify_completed_signal(
    conn,
    signal_uid: str,
    *,
    config: dict[str, Any] | None = None,
    dry_run: bool = False,
) -> int:
    config = config or email_config()
    missing = config_missing(config)
    if missing and not dry_run:
        print(
            json.dumps(
                {
                    "event": "completed_trade_email_not_configured",
                    "signal_uid": signal_uid,
                    "missing": missing,
                }
            ),
            flush=True,
        )
        return 0
    args = argparse.Namespace(
        signal_uid=signal_uid,
        account="",
        instrument="",
        limit=1,
        dry_run=dry_run,
        mark_dry_run=False,
    )
    return process_once(conn, args, config, dry_run or bool(missing))


def main() -> int:
    args = parse_args()
    config = email_config()
    missing = config_missing(config)
    dry_run = bool(args.dry_run or missing)

    conn = wave.connect()
    conn.autocommit(True)
    try:
        ensure_columns(conn)
        if missing:
            print(
                json.dumps(
                    {
                        "event": "smtp_not_configured",
                        "missing": missing,
                        "mode": "dry_run",
                    }
                ),
                flush=True,
            )
        process_once(conn, args, config, dry_run)
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
