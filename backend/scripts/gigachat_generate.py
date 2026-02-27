#!/usr/bin/env python3
import argparse
import os
import sys


def main() -> int:
    try:
        from gigachat import GigaChat
        from gigachat.models import Chat, Messages
        from gigachat.models.messages_role import MessagesRole
    except Exception as exc:
        sys.stderr.write(f"official python sdk 'gigachat' is not available: {exc}\n")
        return 2

    parser = argparse.ArgumentParser()
    parser.add_argument("--topic", required=True)
    parser.add_argument("--grade", required=True)
    parser.add_argument("--count", required=True, type=int)
    parser.add_argument("--model", required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--auth-url", required=True)
    parser.add_argument("--scope", required=True)
    parser.add_argument("--credentials", default="")
    parser.add_argument("--timeout", required=True, type=int)
    parser.add_argument("--system-prompt-file", required=True)
    args = parser.parse_args()

    bearer = os.getenv("BEARER") or os.getenv("GIGACHAT_BEARER")
    credentials = args.credentials.strip() or os.getenv("GIGACHAT_CREDENTIALS")
    if not bearer and not credentials:
        sys.stderr.write("missing access token and credentials\n")
        return 2

    try:
        with open(args.system_prompt_file, "r", encoding="utf-8") as f:
            system_prompt = f.read().strip()
    except Exception as exc:
        sys.stderr.write(f"cannot read system prompt: {exc}\n")
        return 2

    user_prompt = (
        f"Тема: {args.topic}. Класс: {args.grade}. "
        f"Количество вопросов: {max(args.count, 1)}. "
        "Верни только JSON по схеме. У КАЖДОГО вопроса обязательно должно быть поле answer. "
        "Для type=open: answer={\"text\":\"...\"}. Для type=single: answer={\"optionId\":\"...\"}. "
        "Для type=multi: answer={\"optionIds\":[\"...\"]}."
    )

    payload = Chat(
        model=args.model,
        messages=[
            Messages(role=MessagesRole.SYSTEM, content=system_prompt),
            Messages(role=MessagesRole.USER, content=user_prompt),
        ],
        stream=False,
    )

    response = None
    last_error = None

    if bearer:
        try:
            with GigaChat(
                access_token=bearer,
                base_url=args.base_url,
                model=args.model,
                timeout=args.timeout,
                verify_ssl_certs=False,
            ) as giga:
                response = giga.chat(payload)
        except Exception as exc:
            last_error = exc

    if response is None and credentials:
        try:
            with GigaChat(
                credentials=credentials,
                auth_url=args.auth_url,
                scope=args.scope,
                base_url=args.base_url,
                model=args.model,
                timeout=args.timeout,
                verify_ssl_certs=False,
            ) as giga:
                response = giga.chat(payload)
        except Exception as exc:
            last_error = exc

    if response is None:
        sys.stderr.write(f"gigachat request failed: {last_error}\n")
        return 2

    try:
        content = response.choices[0].message.content
    except Exception as exc:
        sys.stderr.write(f"unexpected response shape: {exc}\n")
        return 2

    if not content or not content.strip():
        sys.stderr.write("empty content in response\n")
        return 2

    sys.stdout.write(content.strip())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
