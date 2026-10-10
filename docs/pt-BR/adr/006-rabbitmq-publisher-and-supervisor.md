[English](../../en/adr/006-rabbitmq-publisher-and-supervisor.md)

# ADR 006 — Publisher RabbitMQ confirmado e Supervisor local

Aceita — 2026-10-08, Marco 2.1.

Relay possui observação somente leitura da outbox, sem propriedade nem autoridade de transição. Adapter focado estabelece semântica de publicação sem inventar worker. Management pelo navegador e controle de processos são requisitos locais explícitos.

Usar contrato `EventPublisher` na aplicação com future Send nativa e conexão/canal Lapin 4.12.0 Tokio pertencentes à infraestrutura. Declarar exchange direct/fila/binding duráveis mínimos em configuração da infraestrutura; publicar JSON intacto persistente com identidade estável, mandatory e confirms; rejeitar ack com retorno. Serializar envios em voo para um por adapter. Limitar esperas de conexão/setup/admissão/envio-confirm/close; preservar erros sanitizados com fontes tipadas; retirar adapter após resultado incerto/cancelamento pós-envio e desabilitar retry/recovery automático. Reconexão explícita não reconcilia aceitação anterior desconhecida.

RabbitMQ management 4.3.6 fixo em serviço Compose separado, volume persistente, HTTP management loopback 15672, vhost próprio e usuário management com permissões `relay.*`. Segredos reais só no `.env` ignorado. Variáveis iniciais não reconciliam credenciais de volume existente. Integração no container dispensa AMQP publicado no host. TLS/HA são decisões produtivas posteriores.

Supervisor em primeiro plano sob init Compose como developer, config versionada instalada no caminho padrão, socket Unix privado e único programa `http` executando binário compilado real. Alias executável `supervisor` encaminha argumentos. Stop explícito mantém manager; saída inesperada não zero reinicia filho. SIGTERM drena com orçamento 30s para filho e 35s no Compose; logs do filho sem rotação nos streams.

Alternativas: tipos AMQP na aplicação acoplam orquestração ao transporte; fire-and-forget não estabelece aceitação; retry automático após perda pode duplicar; pool/framework de reconexão acrescentam propriedade especulativa. Filho `cargo run` obscurece ciclo do binário; worker fictício distorce escopo; bancos supervisionados no app duplicam Compose. Serialização troca throughput por correlação limitada e cancelamento simples. Declarar topologia exige configure; futuro deploy pode provisionar com papéis mais restritos. Fila/broker local único são infraestrutura educacional, sem HA produtiva ou processamento exatamente uma vez. Claims, estado, retries e idempotência continuam pré-requisitos de entrega futura.

Veja [implementação, fontes oficiais e validação](../architecture/milestone-2-1.md).
