#!/usr/bin/python3
"""Build SQLx's Docker-internal connection URL without exposing credentials."""
import os
import sys
from urllib.parse import quote


def database_url(environment):
    user = quote(environment["POSTGRES_USER"], safe="")
    password = quote(environment["POSTGRES_PASSWORD"], safe="")
    database = quote(environment["POSTGRES_DB"], safe="")
    host = environment["POSTGRES_HOST"]
    port = environment["POSTGRES_PORT"]
    if host != "postgres" or port != "5432":
        raise ValueError("Docker migration tooling requires postgres:5432")
    return f"postgresql://{user}:{password}@{host}:{port}/{database}"


if __name__ == "__main__":
    environment = os.environ.copy()
    try:
        environment["DATABASE_URL"] = database_url(environment)
    except (KeyError, ValueError):
        sys.exit("sqlx: configure POSTGRES_USER/PASSWORD/DB and postgres:5432 in Compose")
    os.execve("/opt/sqlx/bin/sqlx", ["sqlx", *sys.argv[1:]], environment)
