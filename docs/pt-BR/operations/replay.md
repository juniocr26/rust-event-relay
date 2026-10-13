[Other language](../../en/operations/replay.md)

# Procedimento de replay do operador

REPLAY_OPERATORS ausente/vazio desabilita API privilegiada. /health permanece liveness HTTP independente de prontidão de banco/broker. Migration 20261013000000_add_audited_replay.up.sql só deve ser aplicada após autorização separada de rollout compartilhado. Startup nunca migra: ativação verifica tabela/privilégios somente por leitura e falha sanitizadamente quando ausentes. Worker existente funciona sem migration 5.

## Configuração local

Gerar tokens individuais com CSPRNG do sistema: 32 bytes aleatórios codificados em 64 caracteres hexadecimais minúsculos. Configuração secreta fora de Git/history/logs; injetar no ambiente protegido do processo HTTP:

```json
[{"token":"<64 caracteres hexadecimais de 32 bytes aleatorios>","actor":"local-operator","allow_replay":true,"maximum_budget":10}]
```

REPLAY_OPERATORS contém esse JSON. Até 16 atores/tokens distintos, configuração 8192 bytes, ator 1–64 bytes ASCII alfanuméricos/._- e teto 1–100. Placeholder não é credencial. Configuração não vazia inválida impede startup. Compose transmite variável apenas a app; nunca colocar em VITE_* frontend. Mudança exige ativação/redeployment autorizado, sem hot reload. Manter acesso localhost/rede administrativa; publicação atual do HTTP no host é loopback. Deployment remoto exige TLS/proxy confiável, bloqueio de acesso direto e redaction de Authorization em logs. Administradores confiáveis de processo/container podem acessar secrets do ambiente. Sem cookies/CORS permissivo; header explícito evita autenticação ambiente de cookie/CSRF. Isso não resolve roubo de secret/XSS.

## Requests reproduzíveis

Observar UUID/ciclo/revisão dead_letter e evidência de validação por administração autorizada do banco. Não há endpoint de listagem/inspeção de payload. Escolher UUID de request uma vez e guardar intenção completa antes do envio. Motivo limitado sem secrets/conteúdo, persistido em auditoria restrita.

Arquivo replay.json com IDs/revisão realmente observados; estes valores demonstram sintaxe:

```json
{"request_id":"0199a000-0000-7000-8000-000000000001","event_id":"0199a000-0000-7000-8000-000000000002","expected_revision":4,"expected_cycle":1,"reason":"routing restored after operator review","budget":10,"delay_ms":0,"ticket":null}
```

Usar configuração curl protegida com header Authorization injetado pelo mecanismo de secrets; exemplos sem credenciais e sem tokens em argumentos/URLs:

```sh
curl --config /secure/operator-curl.conf --request POST   --header 'Content-Type: application/json' --data-binary @replay.json   http://127.0.0.1:8080/operator/replays
curl --config /secure/operator-curl.conf   http://127.0.0.1:8080/operator/replays/0199a000-0000-7000-8000-000000000001
```

200 devolve identidades, ciclo, revisão, baseline, orçamento, decided_at e available_at; repetição devolve mesmo recibo. 401 credencial ausente/inválida; 403 permissão ausente/orçamento acima do teto; 400 corpo/campos inválidos; 409 conflito de intenção/estado; 422 conteúdo/metadados/capacidade; 503 falha de persistência ou commit_uncertain_inspect_same_request. GET retorna 404 para ausente/recibo de outro ator. Corpo >4096 bytes rejeitado com 400 sanitizado. GET nunca abre ciclo.

Após cancelamento/rede/COMMIT incerto, consultar mesmo UUID ou reenviar intenção salva idêntica com autorização. Não alterar motivo/orçamento/predicados/UUID para resolver incerteza. 404 pode anteceder commit em andamento. Repetição coordena lock do request, mas lock/statement timeout ainda pode falhar; sem retry interno. Diagnóstico de rejeição não é auditoria transacional.

Recibo reabilita trabalho; observar independentemente banco/auditoria/broker. Worker pode ter avançado estado/revisão; recibo é histórico. Não inferir efeito de consumidor nem desfazer efeitos anteriores. Reparo requer protocolo de autorização/auditoria separado, não implementado aqui.

[ADR 009](../adr/009-authorized-idempotent-replay.md) e [validação/rollout](../architecture/milestone-3-5.md).
