import requests
import sys

API_URL = "https://mcapi.shit.vc/ips?minPlayers=0&limit=500000"

def fetch_ips():
    print("🌐 Fetching IPs from API...")
    print(f"   URL: {API_URL}")
    
    try:
        response = requests.get(API_URL, timeout=30)
        response.raise_for_status()
        
        raw_text = response.text.strip()
        
        ips = [line.strip() for line in raw_text.split('\n') if line.strip()]

        with open("input.txt", "w") as f:
            for ip in ips:
                f.write(f"{ip}\n")
        
        print(f"✅ Saved {len(ips):,} IPs to input.txt")
        return len(ips)
        
    except requests.exceptions.RequestException as e:
        print(f"❌ Error fetching from API: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    count = fetch_ips()
    if count == 0:
        print("⚠️  Warning: No IPs fetched from API")
        sys.exit(1)