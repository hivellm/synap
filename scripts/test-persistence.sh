#!/bin/bash
set -e

cd /mnt/f/Node/hivellm/synap

echo "🧪 Testando Persistência do Synap"
echo "=================================="
echo ""

# 1. Salvar dados
echo "1️⃣ Salvando dados via REST..."
curl -s -X POST http://localhost:15500/kv/set \
  -H 'Content-Type: application/json' \
  -d '{"key":"persist:test1","value":"Teste 1"}' > /dev/null

curl -s -X POST http://localhost:15500/kv/set \
  -H 'Content-Type: application/json' \
  -d '{"key":"persist:test2","value":"Teste 2"}' > /dev/null

curl -s -X POST http://localhost:15500/kv/set \
  -H 'Content-Type: application/json' \
  -d '{"key":"persist:test3","value":"Teste 3"}' > /dev/null

echo "✅ 3 chaves salvas"
echo ""

# 2. Verificar WAL
echo "2️⃣ Verificando WAL..."
ls -lh data/wal/synap.wal
echo ""

# 3. Aguardar fsync
echo "3️⃣ Aguardando 2 segundos (para fsync)..."
sleep 2
ls -lh data/wal/synap.wal
echo ""

# 4. Forçar snapshot
echo "4️⃣ Forçando snapshot..."
curl -s -X POST http://localhost:15500/snapshot | jq .
echo ""

# 5. Listar snapshots
echo "5️⃣ Snapshots disponíveis:"
ls -lht data/snapshots/*.bin | head -3
echo ""

# 6. Ler dados
echo "6️⃣ Lendo dados salvos:"
echo -n "persist:test1 = "
curl -s http://localhost:15500/kv/get/persist:test1
echo ""
echo -n "persist:test2 = "
curl -s http://localhost:15500/kv/get/persist:test2
echo ""

echo ""
echo "✅ Teste completo!"

