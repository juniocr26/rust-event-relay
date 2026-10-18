# Startup e verificação de token


O startup suportado no host é `python3 scripts/start-local.py`. Compila artefatos com `--locked` em `target/runtime`, verifica/faz backup do armazenamento antes de migrações pendentes e inicia Supervisor após os pré-requisitos. Ao iniciar o gerenciador, o entrypoint prepara binários automaticamente; `docker compose up` direto não migra dados retidos. Use o startup suportado em instalações novas. Schema ausente é falha da aplicação ao iniciar, não necessariamente falha de execução do launcher. Consulte `docker compose logs --tail 60 app` e `docker compose exec app supervisorctl status`; não repita restarts nem resete volumes. Paradas intencionais STOPPED continuam respeitadas no mesmo gerenciador. Testes compilam em `target/tests`, sem substituir binários dos serviços.

Dentro de app: `python3 /app/scripts/demo-token.py rotate`. A rotação primeiro salva o token autoritativo, reinicia apenas HTTP em execução para carregá-lo, inicia uma vez programas FATAL/EXITED e preserva STOPPED intencional. Verifica acesso com token novo e rejeição do anterior, estabilidade dos processos, readiness dos processos em execução e topologia durável RabbitMQ via consultas de gerenciamento somente leitura. Não publica eventos nem purga filas. HTTP intencionalmente STOPPED mantém ativação pendente e retorna erro. Se falhar após ativação, informa explicitamente que a rotação já foi aplicada; consulte o token privadamente em `.env` e corrija a falha indicada antes de tentar novamente. Estado de processos Supervisor e topologia de filas são verificações distintas.

---

[README](../../../README.pt-BR.md) | [Catálogo](../index.md) | [English](../../en/operations/demo-token-connectivity.md)

# Token local e conectividade

Inicialização normal na raiz: `python3 scripts/start-local.py`. Reutilize o `.env` privado existente; não o sobrescreva ao clonar. O script compila todos os binários antes das migrações e da inicialização. Retoma processos em FATAL/BACKOFF/EXITED após falha anterior; preserva STOPPED intencional, que exige controle nomeado do Supervisor.

A única fonte do segredo é `DEMO_TOKEN` no `.env` ignorado do host, montado em `/app/.env`. O launcher HTTP lê esse arquivo em cada início e substitui eventual valor herdado antigo. O Compose principal não injeta outra cópia. É um segredo opaco hexadecimal de 64 caracteres, com 256 bits aleatórios seguros, sem expiração automática; não é JWT. Clonar novamente não exige rotação se a configuração privada permanece válida. Nunca use VITE_*, imagem, URL ou armazenamento persistente do navegador para o token.

Dentro do contêiner app em execução, execute exatamente:

```sh
python3 /app/scripts/demo-token.py rotate
```

Equivalente no host: `docker compose exec app python3 /app/scripts/demo-token.py rotate`. Esse comando local explícito grava o novo valor seguro no mesmo `.env` do host, preserva outras configurações e permissões, e reinicia somente o processo nomeado `http` do Supervisor. Verifica HTTP 200 com o novo token e HTTP 401 com o anterior antes de exibir o segredo com aviso de privacidade. Não altera credenciais de PostgreSQL, RabbitMQ ou replay. Mantenha o valor privado. Insira esse mesmo valor em **Local demo token**, escolha **Verify access** e execute o cenário.

O valor persiste após stop/start e recriação do contêiner porque o arquivo do host é a fonte. Abas abertas mantêm o token anterior apenas em memória: precisam receber o novo valor. Conexões HTTP em andamento podem encerrar durante o reinício. Preserve identidades de requests/actions e inspecione antes de reenviar. Relay e consumer continuam ativos; volumes, filas, recibos e dados não são apagados. Se reinício/verificação falhar, o novo valor já está salvo: consulte `supervisorctl status`, corrija a falha e execute `supervisorctl start http`; consulte o valor salvo privadamente no `.env`.

Endereços suportados: `http://localhost:5173` e `http://127.0.0.1:5173`, ou HOST_FRONTEND_PORT configurada. As duas origens exatas são permitidas, sem wildcard. Navegador → proxy Vite `/demo` → `http://app:8080` → rota autenticada. HTTP exige PostgreSQL e migração demo; execução real também exige relay, consumer e RabbitMQ.

401 indica token ausente/inválido; 403 indica Origin/header proibido. 502/proxy ECONNREFUSED indica listener inacessível, não expiração do token. Consulte `docker compose exec -T app supervisorctl status`. Na falha de 17/10/2026, os três processos estavam FATAL por binários ausentes e o proxy registrava ECONNREFUSED. Use o startup normal para compilar e retomar. GET de configuração falho não submeteu trabalho; POST falho pode ter resultado desconhecido e exige inspeção da identidade preservada.

O PDF de auditoria foi comunicação temporária, sem dependência operacional. Consulte [auditoria](final-audit.md), [matriz de aceite](milestone-6-3.md) e [guia da aplicação](application-scenarios.md). Evidências históricas não anulam o defeito de startup reproduzido posteriormente.

## Verificação em 17/10/2026

O proxy principal antes indisponível aceitou as duas origens documentadas após compilação, verificação das migrações e recuperação dos processos nomeados. Um contêiner Chromium local encaminhando ao Vite principal passou 11 verificações: quatro cenários reais, cinco larguras de viewport, ambos os endereços, rejeição de token/Origin e GET de configuração com 502 simulado sem POST nem nova identidade. SQL independente confirmou estoque 10 → 15, um recibo, observações Duplicate correlacionadas e recuperação durável limitada. O token anterior retornou 401 e o novo 200; o novo continuou autorizado após recriação somente de app, com snapshot de runs/actions/recibos/estoque idêntico. Passaram 11 regressões Python, 25 testes de biblioteca Rust (2 não relacionados ignorados) e os 12 testes de integração demo isolados em sequência. Clippy e build de produção frontend passaram. Nenhum segredo configurado foi encontrado nos assets.

A validação ampla inicial encontrou linker encerrado com sinal 9 em builds paralelos e disputa na criação paralela de bancos isolados; repetições sequenciais/limitadas evitam esses conflitos de recursos. Essa evidência é local, sem comprovação de CI remota ou prontidão de produção. Nenhum PDF é exigido.

A campanha descartável do Compose principal passou 29 casos de navegador, com SQL independente, histórico de duplicatas, evidências antigas/desconhecidas, retomada de consumidor pausado e backend indisponível. A instalação limpa passou seis casos focados pelo startup documentado, sem artefatos de build herdados: nove migrações, quatro runs, duas actions e quatro recibos. Stop/start e recriação de app preservaram o mesmo snapshot e token ativo. Serviços/volumes exatos dos testes foram removidos. A validação conferiu 129 arquivos Markdown sem links locais ausentes; os conjuntos de contrapartes Markdown nos dois idiomas coincidem. Âncoras e links remotos não foram verificados.
