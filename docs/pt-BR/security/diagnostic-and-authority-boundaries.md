# Autoridade, diagnóstico e transporte

[English](../../en/security/diagnostic-and-authority-boundaries.md) | [Português brasileiro](diagnostic-and-authority-boundaries.md)

Revisão estática do código: 2026-10-10. Fatos implementados, teoria geral e mudanças hipotéticas são separados abaixo. Comandos runtime não foram executados.

A API HTTP contém apenas `GET /health`, resposta fixa de liveness sem dependências ou login. Tracing JSON estruturado usa `RUST_LOG`. Erros de persistência/publicação expõem categorias sanitizadas em Display/Debug, mas conservam fontes tipadas. Imprimir essas fontes pode revelar conexão, SQL ou payload; formatação externa segura não é redação completa de logs. A revisão não leu credenciais reais ou linhas de aplicação.

Tokens de ownership autorizam futuras transições condicionais no banco, não sigilo de dados. UUID identifica evento estável entre retries; token identifica uma aquisição. Modelos puros rejeitam tokens zero/inválidos, horários incoerentes, owners antigos e transições na expiração exata. Campos nullable da migração precisam estar ausentes juntos ou formar lease pending coerente. Constraints validam formato da linha; não provam histórico de transição, relógio corretamente amostrado ou enforcement concorrente de ownership. Isso exige mutações produtivas e testes concorrentes PostgreSQL, atualmente ausentes.

Transações curtas e publicação fora dos locks são o projeto registrado no ADR 007. Futura completion/release precisa combinar ID, token, estado pending e lease não expirado usando horário do banco após adquirir lock. Publicação no broker é outra fronteira: o token não revoga envio já iniciado. Efeitos tolerantes a duplicatas exigiriam deduplicação do consumidor acoplada a alterações de negócio. É teoria futura, não consumidor implementado. AMQP atual tem transporte em texto puro; armazenamento durável e UUIDs não fornecem cifra de aplicação, assinatura de autenticação ou TLS produtivo.
