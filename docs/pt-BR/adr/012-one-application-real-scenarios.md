Limites atuais e seleção de execuções históricas: [Marco 6.2](../operations/milestone-6-2.md). O limite anterior de uma ação abaixo foi substituído.

[English](../../en/adr/012-one-application-real-scenarios.md) | [Catálogo](../index.md)

# ADR 012 — ambiente único e cenários reais limitados

Substitui a exigência de runtime separado da ADR 011. API no HTTP principal; Supervisor conserva HTTP/relay/consumer; PostgreSQL, RabbitMQ e frontend continuam serviços distintos. Backup protegido, restauração descartável validada e checksums precedem migrations forward. Exportar evidências antigas, parar apenas containers cuja propriedade foi conferida e manter volumes.

Quatro intenções fixas: normal, dedupe, concurrent e recovery. Produtor mantém transação única, estoque inicial 10 e evento de 5 unidades. Uma ação durável por run, idempotente por request, com 1 ou 3 republicações do envelope imutável. Ações queued retomam; executing interrompidas tornam-se unknown, sem republicação automática. Deadline 20s e até três chamadas de publicação. Nenhuma promessa de exatamente duas/quatro entregas sob incerteza.

Mesmo broker/vhost/exchange, fila durável própria `relay.demo.<action UUID>` e um ou dois contextos temporários com prefetch 1; não aumentar limites do consumidor principal. Barreira antes dos primeiros handlers concorrentes. Pool do executor 2, execução global single-flight. Fechamento/cancelamento encerra sessões; fila e dados permanecem inspecionáveis. No máximo 100 runs/filas e uma ação por run.

Observações Applied/Duplicate de eventos demo commitam na transação do consumidor, limitadas a 32 por run sob lock. Duplicate não altera estoque. Observações de publicação são opcionais, limitadas e não mudam garantia de entrega; horário observado não é timestamp exato do broker. Falha recovery rejeita uma vez antes do envio e usa retry/ownership reais, com registro durável da condição e agenda/revisão no mesmo commit do agendamento. Não é indisponibilidade da infraestrutura nem NACK do broker.

Token separado, Origin exata, sem credencial no bundle, CORS/cookies ou endpoint administrativo. Polling sequencial 1s/60s, identidade preservada em respostas incertas; fechar para observação, não trabalho commitado. Playback de crash/race/ACK continua Illustrative. Sem promessa de produção ou transporte exatamente uma vez. [Contrato detalhado](../../en/adr/012-one-application-real-scenarios.md).
