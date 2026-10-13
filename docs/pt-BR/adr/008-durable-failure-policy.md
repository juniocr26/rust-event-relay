[Other language](../../en/adr/008-durable-failure-policy.md)

# ADR 008 — Política durável de falhas e contratos de recuperação

Status: aceito; persistência 3.2 e integração da aplicação 3.3 implementadas; rollout compartilhado planejado. Data: 2026-10-12.

## Contexto

Marco 2 preserva leases, contadores de aquisição e efeitos ambíguos. Pacing volátil não implementa agenda durável. Categorias não determinam permanência ou segurança de retry. Dados inválidos nem sempre formam OwnedOutboxEvent.

## Decisão

Adotar a [política e matriz completas](../architecture/milestone-3-1.md): dez aquisições confirmadas por ciclo, contador vitalício preservado, baseline zero nas linhas existentes e novo ciclo explícito no replay. Relógio PostgreSQL pós-lock, microssegundos com aritmética verificada e equal jitter limitado (base 1 s, teto 300 s). Reagendar somente falhas definitivas elegíveis pelo contexto e autoridade ativa. Recuperar infraestrutura não prova invalidade permanente. Efeitos incertos retêm lease e exigem observação/expiração; reconciliação genérica não existe atualmente.

Preservar evento terminal em dead_letter com diagnóstico/evidência/auditoria sanitizados. Manutenção de linha bruta trata inválidos/esgotados protegendo donos ativos. Replay exige intenção explícita, revisão esperada, request auditável durável e identidade original; não promete repetir efeito do consumidor.

## Alternativas e consequências

Orçamento de publicações exige protocolo durável adicional e não mede aceite externo atomicamente. Aquisições são solução menor auditável, cobrando crash/outage antes do envio. Tabela separada favorece separação de acesso mas exige movimentação/consistência; preservar linha e complementar evidência. Release imediato incerto acelera duplicações possíveis; retenção adia recuperação sem eliminar duplicatas. Dez aquisições é padrão operacional finito, não prova poison; preservar motivo e permitir replay auditado.

3.1 implementa somente cálculos puros de orçamento/backoff e testes, sem migrations ou integração. Critérios 3.2–3.6 e pré-requisitos estão na política. Garantias ADR 007 preservadas.

Atualização de implementação (12/10/2026): transições persistentes/testes isolados de 3.2 implementados; integração do pipeline e rollout compartilhado continuam pendentes. [Marco 3.2](../architecture/milestone-3-2.md).

Atualização (2026-10-13): [integração 3.3](../architecture/milestone-3-3.md) implementada. Serialização sem contexto retém lease; atestado permanente explícito autoriza preservação. Unavailable mantém liberação antes do envio conforme ADR somente enquanto houver orçamento; falha definitiva final preserva esgotamento. Rollout compartilhado permanece pendente.
