set -e

cd "$(dirname "$0")"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║         Minecraft Server Scanning Pipeline                ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

echo "📡 PHASE 1: Discovery"
echo "─────────────────────────────────────────────────────────────"

echo "→ Adjusting system limits..."
ulimit -n 65535 
echo "✓ File descriptor limit: $(ulimit -n)"

echo "→ Fetching IPs from API..."
python3 fetch_api_ips.py

echo "→ Sending start message"
python start.py

echo "→ Finding new IPs to check..."
if [ -f "ips.txt" ] && [ -f "input.txt" ]; then
    grep -Fxv -f input.txt ips.txt > check.txt || true
    CHECK_COUNT=$(wc -l < check.txt)
    echo "✓ Found ${CHECK_COUNT} new IPs to scan"
else
    echo "⚠️  Warning: ips.txt not found, skipping comparison"
    cp input.txt check.txt
fi

if [ ! -s check.txt ]; then
    echo ""
    echo "⚠️  No new IPs to scan. Skipping ping phase."
else
    echo ""
    echo "🔍 PHASE 2: Fast Ping Scan"
    echo "─────────────────────────────────────────────────────────────"
    
    echo "→ Running fast Minecraft server detection..."
    ./rust/target/release/mcping
    
    if [ -f "mc.txt" ]; then
        MC_COUNT=$(wc -l < mc.txt)
        echo "✓ Found ${MC_COUNT} Minecraft servers"
    else
        echo "⚠️  No Minecraft servers found"
        touch mc.txt
    fi
fi

if [ ! -s mc.txt ]; then
    echo ""
    echo "⚠️  No servers to probe. Skipping auth scan."
else
    echo ""
    echo "🔐 PHASE 3: Auth Mode Detection"
    echo "─────────────────────────────────────────────────────────────"
    
    cp mc.txt input.txt
    
    echo "→ Running detailed scan with auth detection..."
    ./rust/target/release/mcprobe_auth
    
    echo "→ Importing results with auth mode..."
    python3 import_auth.py results.json
    
    echo "→ Cleaning up auth results..."
    rm -f results.json
    rm -f mc.txt
    rm -f input.txt
    rm -f check.txt
fi

echo ""
echo "🔄 PHASE 4: Normal Scan Pipeline"
echo "─────────────────────────────────────────────────────────────"

echo "→ Re-fetching fresh IPs from API for normal scan..."
python3 fetch_api_ips.py

echo "→ Running start.py..."
python3 start.py

echo "→ Running standard mcprobe..."
./rust/target/release/mcprobe

echo "→ Importing standard results..."
python3 import.py results.json
rm -f results.json
rm -f input.txt

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                 Pipeline Complete! ✅                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

if [ -f "mc.txt" ]; then
    echo "📊 Summary:"
    echo "   • New servers discovered: $(wc -l < mc.txt 2>/dev/null || echo 0)"
fi

echo ""
echo "All tasks completed successfully."