#!/bin/bash
set -euo pipefail

# Deploy lean-devnet to a local kind cluster with peer discovery.
# Usage: ./scripts/deploy-local.sh

NAMESPACE="lean-devnet"
CONTEXT="kind-lean-devnet"
GENESIS_DIR="/tmp/lean-devnet-genesis/genesis"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
QUICKSTART_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Step 1: Ensure StatefulSets are running (without peering)..."
kubectl --context "$CONTEXT" scale statefulset ethlambda ream -n "$NAMESPACE" --replicas=1 2>/dev/null || true
sleep 5

echo "==> Step 2: Wait for pods to be Running..."
kubectl --context "$CONTEXT" wait --for=condition=ready pod/ethlambda-0 -n "$NAMESPACE" --timeout=60s
kubectl --context "$CONTEXT" wait --for=condition=ready pod/ream-0 -n "$NAMESPACE" --timeout=60s

echo "==> Step 3: Capture pod IPs..."
ETHLAMBDA_IP=$(kubectl --context "$CONTEXT" get pod ethlambda-0 -n "$NAMESPACE" -o jsonpath='{.status.podIP}')
REAM_IP=$(kubectl --context "$CONTEXT" get pod ream-0 -n "$NAMESPACE" -o jsonpath='{.status.podIP}')
echo "  ethlambda-0: $ETHLAMBDA_IP"
echo "  ream-0:      $REAM_IP"

echo "==> Step 4: Patch validator-config.yaml with real IPs..."
cp "$GENESIS_DIR/validator-config.yaml" "$GENESIS_DIR/validator-config.yaml.bak"
# Reset IPs to placeholder, then set real ones
sed -i.tmp "s|ip: .*|ip: PLACEHOLDER|" "$GENESIS_DIR/validator-config.yaml"
# First occurrence = ethlambda, second = ream
awk -v ip1="$ETHLAMBDA_IP" -v ip2="$REAM_IP" '
  BEGIN{n=0}
  /ip: PLACEHOLDER/{n++; if(n==1) sub(/PLACEHOLDER/,ip1); else sub(/PLACEHOLDER/,ip2)}
  1
' "$GENESIS_DIR/validator-config.yaml" > "$GENESIS_DIR/validator-config-patched.yaml"
mv "$GENESIS_DIR/validator-config-patched.yaml" "$GENESIS_DIR/validator-config.yaml"
rm -f "$GENESIS_DIR/validator-config.yaml.tmp"

echo "==> Step 5: Regenerate genesis with real IPs..."
rm -f "$GENESIS_DIR"/{config.yaml,genesis.ssz,genesis.json,nodes.yaml,validators.yaml,annotated_validators.yaml}
SKIP_KEY_GEN=true "$QUICKSTART_DIR/generate-genesis.sh" "$GENESIS_DIR" 2>&1 | grep -E "^(✅|🔧|🔑|✓)" || true

echo "==> Step 6: Update ConfigMap..."
kubectl --context "$CONTEXT" delete configmap genesis-config -n "$NAMESPACE" 2>/dev/null || true
kubectl --context "$CONTEXT" create configmap genesis-config -n "$NAMESPACE" \
  --from-file=config.yaml="$GENESIS_DIR/config.yaml" \
  --from-file=validators.yaml="$GENESIS_DIR/validators.yaml" \
  --from-file=annotated_validators.yaml="$GENESIS_DIR/annotated_validators.yaml" \
  --from-file=nodes.yaml="$GENESIS_DIR/nodes.yaml" \
  --from-file=genesis.json="$GENESIS_DIR/genesis.json" \
  --from-file=genesis.ssz="$GENESIS_DIR/genesis.ssz" \
  --from-file=validator-config.yaml="$GENESIS_DIR/validator-config.yaml" \
  --from-file=ethlambda_0.key="$GENESIS_DIR/ethlambda_0.key" \
  --from-file=ream_0.key="$GENESIS_DIR/ream_0.key"

echo "==> Step 7: Restart pods to pick up new config..."
kubectl --context "$CONTEXT" delete pod ethlambda-0 ream-0 -n "$NAMESPACE"
sleep 3

echo "==> Step 8: Wait for pods and verify IPs are stable..."
kubectl --context "$CONTEXT" wait --for=condition=ready pod/ethlambda-0 -n "$NAMESPACE" --timeout=60s
kubectl --context "$CONTEXT" wait --for=condition=ready pod/ream-0 -n "$NAMESPACE" --timeout=60s

NEW_ETHLAMBDA_IP=$(kubectl --context "$CONTEXT" get pod ethlambda-0 -n "$NAMESPACE" -o jsonpath='{.status.podIP}')
NEW_REAM_IP=$(kubectl --context "$CONTEXT" get pod ream-0 -n "$NAMESPACE" -o jsonpath='{.status.podIP}')

if [ "$NEW_ETHLAMBDA_IP" != "$ETHLAMBDA_IP" ] || [ "$NEW_REAM_IP" != "$REAM_IP" ]; then
  echo "WARNING: Pod IPs changed after restart!"
  echo "  ethlambda: $ETHLAMBDA_IP -> $NEW_ETHLAMBDA_IP"
  echo "  ream:      $REAM_IP -> $NEW_REAM_IP"
  echo "Re-running with new IPs..."
  exec "$0"
fi

echo ""
echo "==> Devnet is running with peer discovery!"
echo "  ethlambda-0: $ETHLAMBDA_IP"
echo "  ream-0:      $REAM_IP"
echo ""
echo "  kubectl --context $CONTEXT logs -f ethlambda-0 -n $NAMESPACE"
echo "  kubectl --context $CONTEXT logs -f ream-0 -n $NAMESPACE"
