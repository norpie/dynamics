#!/bin/bash

# Dynamics 365 API Query Wrapper with OAuth Authentication
# Usage: ./dynamics-api.sh <endpoint> [-m method] [-d data] [-o output_file] [-h host_num] [-a] [--max-pages N]
# Examples:
#   ./dynamics-api.sh "adx_entitylists?\$filter=adx_name eq 'Active Projects'"
#   ./dynamics-api.sh "nrq_requests(guid)"
#   ./dynamics-api.sh "nrq_requests(guid)" -m PATCH -d '{"statuscode": 2}'
#   ./dynamics-api.sh "nrq_requests(guid)" -o response.json
#   ./dynamics-api.sh "nrq_requests" -h 2  # Uses DYNAMICS_HOST2
#   ./dynamics-api.sh "nrq_projects" -a    # Follow all @odata.nextLink pages
#   ./dynamics-api.sh "nrq_projects" -a --max-pages 10 -o all.json

# Load environment variables from .env file if it exists
if [ -f .env ]; then
  set -a
  source .env
  set +a
fi

# Configuration
DYNAMICS_CLIENT_ID="${DYNAMICS_CLIENT_ID:-}"
DYNAMICS_CLIENT_SECRET="${DYNAMICS_CLIENT_SECRET:-}"
DYNAMICS_USERNAME="${DYNAMICS_USERNAME:-}"
DYNAMICS_PASSWORD="${DYNAMICS_PASSWORD:-}"
API_VERSION="${DYNAMICS_API_VERSION:-v9.2}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Parse arguments
ENDPOINT="${1:-}"
METHOD="GET"
DATA=""
OUTPUT_FILE=""
HOST_NUM=""
CUSTOM_HEADERS=()
FOLLOW_ALL=false
MAX_PAGES=""
PAGE_SIZE=""

# Check for flags in arguments
for ((j=1; j<=$#; j++)); do
  case "${!j}" in
    -o)
      k=$((j+1))
      OUTPUT_FILE="${!k}"
      ;;
    -h)
      k=$((j+1))
      HOST_NUM="${!k}"
      ;;
    -m)
      k=$((j+1))
      METHOD="${!k}"
      ;;
    -d)
      k=$((j+1))
      DATA="${!k}"
      ;;
    -H)
      k=$((j+1))
      CUSTOM_HEADERS+=("${!k}")
      ;;
    -a|--all)
      FOLLOW_ALL=true
      ;;
    --max-pages)
      k=$((j+1))
      MAX_PAGES="${!k}"
      ;;
    --page-size|-p)
      k=$((j+1))
      PAGE_SIZE="${!k}"
      ;;
  esac
done

# Determine which host variable to use
if [ -z "$HOST_NUM" ] || [ "$HOST_NUM" = "1" ]; then
  HOST_VAR="DYNAMICS_HOST"
else
  HOST_VAR="DYNAMICS_HOST${HOST_NUM}"
fi
DYNAMICS_HOST="${!HOST_VAR:-}"

# Check for required configuration
if [ -z "$DYNAMICS_HOST" ]; then
  echo -e "${RED}ERROR: $HOST_VAR not set${NC}"
  echo ""
  echo "Please set the following variables in .env:"
  echo "  DYNAMICS_HOST=https://your-org.crm.dynamics.com"
  echo "  DYNAMICS_HOST2=https://your-other-org.crm.dynamics.com  (optional)"
  echo "  DYNAMICS_CLIENT_ID=your-client-id"
  echo "  DYNAMICS_CLIENT_SECRET=your-client-secret"
  echo "  DYNAMICS_USERNAME=your-username"
  echo "  DYNAMICS_PASSWORD=your-password"
  echo ""
  exit 1
fi

if [ -z "$DYNAMICS_CLIENT_ID" ] || [ -z "$DYNAMICS_CLIENT_SECRET" ] || [ -z "$DYNAMICS_USERNAME" ] || [ -z "$DYNAMICS_PASSWORD" ]; then
  echo -e "${RED}ERROR: Missing required authentication credentials${NC}"
  echo "Please check .env file"
  exit 1
fi

if [ -z "$ENDPOINT" ]; then
  echo -e "${RED}ERROR: No endpoint provided${NC}"
  echo ""
  echo "Usage: $0 <endpoint> [-m method] [-d data] [-o output_file] [-h host_num]"
  echo ""
  echo "Options:"
  echo "  -m METHOD    HTTP method (GET, POST, PATCH, DELETE). Default: GET"
  echo "  -d DATA      JSON data for POST/PATCH requests"
  echo "  -o FILE      Save response to file"
  echo "  -h NUM       Use alternate host (DYNAMICS_HOST2, DYNAMICS_HOST3, etc.)"
  echo "  -H HEADER    Add custom header (can be used multiple times)"
  echo "  -a, --all    Follow all @odata.nextLink pages automatically"
  echo "  --max-pages N  Limit pagination to N pages (requires -a)"
  echo "  -p, --page-size N  Set page size via Prefer header (default: server decides)"
  echo ""
  echo "Examples:"
  echo "  # Get entity list"
  echo "  $0 \"adx_entitylists?\\\$filter=adx_name eq 'Active Projects'\""
  echo ""
  echo "  # Get specific request"
  echo "  $0 \"nrq_requests(b1a679d1-df19-f011-998a-7c1e52527787)\""
  echo ""
  echo "  # Get with OData query"
  echo "  $0 \"nrq_projects?\\\$select=nrq_name,nrq_projectid&\\\$top=5\""
  echo ""
  echo "  # Update (PATCH) a record"
  echo "  $0 \"nrq_requests(guid)\" -m PATCH -d '{\"statuscode\": 2}'"
  echo ""
  echo "  # Create (POST) a record"
  echo "  $0 \"nrq_projects\" -m POST -d '{\"nrq_name\": \"Test Project\"}'"
  echo ""
  echo "  # Save output to file"
  echo "  $0 \"nrq_projects(guid)\" -o project.json"
  echo ""
  echo "  # Use alternate host (DYNAMICS_HOST2)"
  echo "  $0 \"nrq_projects\" -h 2"
  echo ""
  echo "  # Fetch all pages automatically"
  echo "  $0 \"nrq_projects\" -a"
  echo ""
  echo "  # Fetch all pages, save to file"
  echo "  $0 \"nrq_projects\" -a -o all_projects.json"
  echo ""
  echo "  # Fetch with page limit"
  echo "  $0 \"nrq_projects\" -a --max-pages 5"
  echo ""
  echo "Common OData operators:"
  echo "  \$filter  - Filter results (eq, ne, gt, lt, contains, startswith)"
  echo "  \$select  - Select specific fields"
  echo "  \$expand  - Expand related entities"
  echo "  \$top     - Limit number of results"
  echo "  \$orderby - Sort results"
  echo ""
  exit 1
fi

# Function to get OAuth access token
get_access_token() {
  local TOKEN_URL="https://login.windows.net/common/oauth2/token"

  # Check if we have a cached token
  local CACHE_FILE="./.token_cache_${HOST_VAR}"
  if [ -f "$CACHE_FILE" ]; then
    local CACHED_TOKEN=$(cat "$CACHE_FILE" | jq -r '.access_token')
    local EXPIRES_AT=$(cat "$CACHE_FILE" | jq -r '.expires_at')
    local CURRENT_TIME=$(date +%s)

    # Check if token is still valid (with 30 sec buffer)
    if [ "$CURRENT_TIME" -lt "$((EXPIRES_AT - 30))" ]; then
      echo "$CACHED_TOKEN"
      return 0
    fi
  fi

  # Get new token
  local RESPONSE=$(curl -s -X POST "$TOKEN_URL" \
    -d "grant_type=password" \
    -d "client_id=$DYNAMICS_CLIENT_ID" \
    -d "client_secret=$DYNAMICS_CLIENT_SECRET" \
    -d "username=$DYNAMICS_USERNAME" \
    -d "password=$DYNAMICS_PASSWORD" \
    -d "resource=$DYNAMICS_HOST")

  # Check if request was successful
  if echo "$RESPONSE" | jq -e '.access_token' > /dev/null 2>&1; then
    local TOKEN=$(echo "$RESPONSE" | jq -r '.access_token')
    local EXPIRES_IN=$(echo "$RESPONSE" | jq -r '.expires_in // 3600')
    local EXPIRES_AT=$(($(date +%s) + EXPIRES_IN))

    # Cache the token
    echo "{\"access_token\":\"$TOKEN\",\"expires_at\":$EXPIRES_AT}" > "$CACHE_FILE"

    echo "$TOKEN"
    return 0
  else
    echo -e "${RED}ERROR: Failed to get access token${NC}" >&2
    echo "Response: $RESPONSE" >&2
    return 1
  fi
}

# Get access token
echo -e "${CYAN}Authenticating...${NC}" >&2
TOKEN=$(get_access_token)
if [ $? -ne 0 ]; then
  exit 1
fi
echo -e "${GREEN}✓ Authenticated${NC}" >&2
echo "" >&2

# Build full URL
if [[ "$ENDPOINT" == http* ]]; then
  # Full URL provided (e.g., from @odata.nextLink) - use as-is, no encoding
  FULL_URL="$ENDPOINT"
else
  # URL encode problematic characters for relative endpoints
  # Note: percent must be encoded first to avoid double-encoding
  ENDPOINT="${ENDPOINT//%/%25}"      # Percent (must be first!)
  ENDPOINT="${ENDPOINT// /%20}"      # Space
  ENDPOINT="${ENDPOINT//#/%23}"      # Hash
  ENDPOINT="${ENDPOINT//+/%2B}"      # Plus

  if [[ "$ENDPOINT" == /api/* ]]; then
    FULL_URL="${DYNAMICS_HOST}${ENDPOINT}"
  else
    FULL_URL="${DYNAMICS_HOST}/api/data/${API_VERSION}/${ENDPOINT#/}"
  fi
fi

# Start timer
START_TIME=$(date +%s%N)

# Print request info
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
echo -e "${YELLOW}$METHOD${NC} ${BLUE}$FULL_URL${NC}" >&2
if [ -n "$DATA" ]; then
  echo -e "${CYAN}Data:${NC}" >&2
  echo "$DATA" | jq '.' 2>/dev/null || echo "$DATA" >&2
fi
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
echo "" >&2

# Build curl command based on method
CURL_CMD=(curl -s -w "\n%{http_code}")

# Add headers
CURL_CMD+=(-H "Authorization: Bearer $TOKEN")
CURL_CMD+=(-H "Accept: application/json")
CURL_CMD+=(-H "OData-MaxVersion: 4.0")
CURL_CMD+=(-H "OData-Version: 4.0")

# Add page size header if specified
if [ -n "$PAGE_SIZE" ]; then
  CURL_CMD+=(-H "Prefer: odata.maxpagesize=$PAGE_SIZE")
fi

# Add custom headers
for header in "${CUSTOM_HEADERS[@]}"; do
  CURL_CMD+=(-H "$header")
done

# Add method and data if applicable
case "$METHOD" in
  GET)
    CURL_CMD+=(-X GET)
    ;;
  POST)
    CURL_CMD+=(-X POST)
    CURL_CMD+=(-H "Content-Type: application/json; charset=utf-8")
    if [ -n "$DATA" ]; then
      CURL_CMD+=(-d "$DATA")
    fi
    ;;
  PATCH)
    CURL_CMD+=(-X PATCH)
    CURL_CMD+=(-H "Content-Type: application/json; charset=utf-8")
    if [ -n "$DATA" ]; then
      CURL_CMD+=(-d "$DATA")
    fi
    ;;
  DELETE)
    CURL_CMD+=(-X DELETE)
    ;;
  *)
    echo -e "${RED}ERROR: Unsupported method '$METHOD'${NC}" >&2
    echo "Supported methods: GET, POST, PATCH, DELETE" >&2
    exit 1
    ;;
esac

# Add URL
CURL_CMD+=("$FULL_URL")

# Execute request
RESPONSE=$("${CURL_CMD[@]}")

# Parse response
HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
RESPONSE_DATA=$(echo "$RESPONSE" | sed '$d')

# End timer
END_TIME=$(date +%s%N)
DURATION=$(( (END_TIME - START_TIME) / 1000000 ))

# Check for failure on initial request (fail-fast)
if [ "$HTTP_CODE" -lt 200 ] || [ "$HTTP_CODE" -ge 300 ]; then
  echo -e "${CYAN}Response (HTTP $HTTP_CODE):${NC}" >&2
  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
  echo "$RESPONSE_DATA" | jq '.' 2>/dev/null || echo "$RESPONSE_DATA"
  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
  echo -e "${RED}✗ Error${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC}" >&2
  exit 1
fi

# Handle pagination if -a flag is set
if [ "$FOLLOW_ALL" = true ] && [ "$METHOD" = "GET" ]; then
  PAGE_NUM=1
  ALL_VALUES=$(echo "$RESPONSE_DATA" | jq '.value // []')
  CONTEXT=$(echo "$RESPONSE_DATA" | jq -r '.["@odata.context"] // empty')
  NEXT_LINK=$(echo "$RESPONSE_DATA" | jq -r '.["@odata.nextLink"] // empty')

  INITIAL_COUNT=$(echo "$ALL_VALUES" | jq 'length')
  echo -e "${GREEN}✓ Page 1${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC} - ${INITIAL_COUNT} records" >&2

  while [ -n "$NEXT_LINK" ]; do
    # Check max pages limit
    if [ -n "$MAX_PAGES" ] && [ "$PAGE_NUM" -ge "$MAX_PAGES" ]; then
      echo -e "${YELLOW}⚠ Stopped at page limit ($MAX_PAGES), more data may exist${NC}" >&2
      break
    fi

    PAGE_NUM=$((PAGE_NUM + 1))
    echo -e "${CYAN}Fetching page $PAGE_NUM...${NC}" >&2

    # Fetch next page
    PAGE_START=$(date +%s%N)
    PAGE_CURL_CMD=(curl -s -w "\n%{http_code}" \
      -H "Authorization: Bearer $TOKEN" \
      -H "Accept: application/json" \
      -H "OData-MaxVersion: 4.0" \
      -H "OData-Version: 4.0")
    if [ -n "$PAGE_SIZE" ]; then
      PAGE_CURL_CMD+=(-H "Prefer: odata.maxpagesize=$PAGE_SIZE")
    fi
    PAGE_CURL_CMD+=("$NEXT_LINK")
    PAGE_RESPONSE=$("${PAGE_CURL_CMD[@]}")

    PAGE_HTTP_CODE=$(echo "$PAGE_RESPONSE" | tail -n1)
    PAGE_DATA=$(echo "$PAGE_RESPONSE" | sed '$d')
    PAGE_END=$(date +%s%N)
    PAGE_DURATION=$(( (PAGE_END - PAGE_START) / 1000000 ))

    # Fail-fast on error
    if [ "$PAGE_HTTP_CODE" -lt 200 ] || [ "$PAGE_HTTP_CODE" -ge 300 ]; then
      echo -e "${RED}✗ Failed on page $PAGE_NUM${NC} (HTTP $PAGE_HTTP_CODE) ${CYAN}⏱ ${PAGE_DURATION}ms${NC}" >&2
      echo "$PAGE_DATA" | jq '.' 2>/dev/null || echo "$PAGE_DATA" >&2
      exit 1
    fi

    # Merge values
    PAGE_VALUES=$(echo "$PAGE_DATA" | jq '.value // []')
    PAGE_COUNT=$(echo "$PAGE_VALUES" | jq 'length')
    ALL_VALUES=$(echo "$ALL_VALUES $PAGE_VALUES" | jq -s 'add')

    echo -e "${GREEN}✓ Page $PAGE_NUM${NC} (HTTP $PAGE_HTTP_CODE) ${CYAN}⏱ ${PAGE_DURATION}ms${NC} - ${PAGE_COUNT} records" >&2

    # Get next link
    NEXT_LINK=$(echo "$PAGE_DATA" | jq -r '.["@odata.nextLink"] // empty')
  done

  # Build combined response
  TOTAL_COUNT=$(echo "$ALL_VALUES" | jq 'length')
  if [ -n "$CONTEXT" ]; then
    RESPONSE_DATA=$(echo "$ALL_VALUES" | jq --arg ctx "$CONTEXT" '{"@odata.context": $ctx, "value": .}')
  else
    RESPONSE_DATA=$(echo "$ALL_VALUES" | jq '{"value": .}')
  fi

  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
  echo -e "${GREEN}✓ Complete${NC} - ${TOTAL_COUNT} total records from ${PAGE_NUM} page(s)" >&2
  echo "" >&2
else
  # Non-paginated output
  echo -e "${CYAN}Response (HTTP $HTTP_CODE):${NC}" >&2
  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2
  echo -e "${GREEN}✓ Success${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC}" >&2

  # Show record count for GET requests with 'value' array
  if [ "$METHOD" = "GET" ]; then
    RECORD_COUNT=$(echo "$RESPONSE_DATA" | jq '.value | length' 2>/dev/null)
    if [ -n "$RECORD_COUNT" ] && [ "$RECORD_COUNT" != "null" ]; then
      echo -e "${CYAN}Records returned: $RECORD_COUNT${NC}" >&2
    fi
  fi
  echo "" >&2
fi

# Output final response
if [ -n "$OUTPUT_FILE" ]; then
  # Try to format as JSON before saving
  if echo "$RESPONSE_DATA" | jq '.' > "$OUTPUT_FILE" 2>/dev/null; then
    echo -e "${GREEN}✓ Response saved to: $OUTPUT_FILE${NC}" >&2
  else
    # Not JSON or jq failed, save raw
    echo "$RESPONSE_DATA" > "$OUTPUT_FILE"
    echo -e "${GREEN}✓ Response saved to: $OUTPUT_FILE${NC}" >&2
  fi
else
  # Try to format as JSON, fall back to raw output
  if echo "$RESPONSE_DATA" | jq '.' 2>/dev/null; then
    : # jq succeeded, output already displayed
  else
    # Not JSON or jq failed, show raw
    echo "$RESPONSE_DATA"
  fi
fi

# Success (failures exit early above)
exit 0
