[English](../../en/adr/011-persistent-demo-runs.md) | [Catálogo](../index.md)

# ADR 011: execuções locais com evidência persistente

Decisão do Marco 6.1: stack `relay-demo61` com banco, broker e volumes próprios. O contrato detalhado está na [versão inglesa](../../en/adr/011-persistent-demo-runs.md).

Uma transação cria execução, identidade idempotente, fato de recebimento de 5 unidades, estoque inicial 10 e outbox canônico. O produtor não aplica o incremento do consumidor. Repetir identidade e intenção retorna a mesma execução; divergência gera conflito. Perda da confirmação de COMMIT é resultado desconhecido: consultar ou reenviar a mesma identidade, sem gerar outra automaticamente.

Evidências independentes: publicação concluída no outbox, receipt do consumidor e estoque atual. Não inferir lease, instante de confirmação do broker ou recebimento de ACK. Erro HTTP não prova falha da execução. Fechar o modal apenas interrompe observação.

Token exclusivo da demonstração em memória no navegador, sem VITE_*; leituras e escritas autenticadas, Origin exata, header próprio e ausência de CORS/cookies. Limites: 100 registros retidos, 4 execuções pendentes, 2 requisições simultâneas, 10 requisições/s, body 1024 bytes, deadline HTTP 8s, pool 2, polling sequencial 1s por 60s. Sem expiração ou descarte automático.

Parada normal preserva volumes. Limpeza manual exige parar aplicações, remover somente o volume do broker dedicado para impedir redelivery e depois o volume SQL dedicado; todas as evidências dessa stack serão perdidas. Nunca remover receipts isoladamente. Marco 6.2 exige publicação real repetida do envelope imutável e observação correlacionada de Duplicate; recuperação ampliada fica para 6.3. Sem promessa de transporte exatamente uma vez ou produção.
