#!/bin/bash

# SharePoint API Query Wrapper with OAuth Authentication
# Usage: ./sharepoint-api.sh <endpoint> [-m method] [-d data] [-o output_file] [-g]
# Examples:
#   ./sharepoint-api.sh "sites/root"
#   ./sharepoint-api.sh "sites/{site-id}/lists"
#   ./sharepoint-api.sh "sites/{site-id}/drive/root/children"
#   ./sharepoint-api.sh "/sites/{tenant}.sharepoint.com:/sites/MySite:/lists" -g
#   ./sharepoint-api.sh "sites/{site-id}/lists/{list-id}/items" -m POST -d '{"fields": {"Title": "New Item"}}'

# Load environment variables from .env file if it exists
if [ -f .env ]; then
  set -a
  source .env
  set +a
fi

# Configuration
SHAREPOINT_CLIENT_ID="${SHAREPOINT_CLIENT_ID:-}"
SHAREPOINT_CLIENT_SECRET="${SHAREPOINT_CLIENT_SECRET:-}"
SHAREPOINT_USERNAME="${SHAREPOINT_USERNAME:-}"
SHAREPOINT_PASSWORD="${SHAREPOINT_PASSWORD:-}"
SHAREPOINT_SITE_URL="${SHAREPOINT_SITE_URL:-}"

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
USE_GRAPH=true  # Default to Microsoft Graph API
CUSTOM_HEADERS=()
OVERRIDE_SITE_URL=""  # Override SHAREPOINT_SITE_URL for this request

# Check for flags in arguments
for ((j=1; j<=$#; j++)); do
  case "${!j}" in
    -o)
      k=$((j+1))
      OUTPUT_FILE="${!k}"
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
    -s|--sharepoint-rest)
      USE_GRAPH=false
      ;;
    --site-url)
      k=$((j+1))
      OVERRIDE_SITE_URL="${!k}"
      ;;
  esac
done

# Check for required configuration
if [ -z "$SHAREPOINT_CLIENT_ID" ] || [ -z "$SHAREPOINT_CLIENT_SECRET" ] || [ -z "$SHAREPOINT_USERNAME" ] || [ -z "$SHAREPOINT_PASSWORD" ]; then
  echo -e "${RED}ERROR: Missing required authentication credentials${NC}"
  echo ""
  echo "Please set the following variables in .env:"
  echo "  SHAREPOINT_CLIENT_ID=your-app-client-id"
  echo "  SHAREPOINT_CLIENT_SECRET=your-app-client-secret"
  echo "  SHAREPOINT_USERNAME=your-username"
  echo "  SHAREPOINT_PASSWORD=your-password"
  echo "  SHAREPOINT_SITE_URL=https://yourtenant.sharepoint.com/sites/yoursite (optional, for -s flag)"
  echo ""
  exit 1
fi

if [ -z "$ENDPOINT" ]; then
  echo -e "${RED}ERROR: No endpoint provided${NC}"
  echo ""
  echo "Usage: $0 <endpoint> [-m method] [-d data] [-o output_file] [-s] [--site-url url]"
  echo ""
  echo "Options:"
  echo "  -m METHOD       HTTP method (GET, POST, PATCH, DELETE). Default: GET"
  echo "  -d DATA         JSON data for POST/PATCH requests"
  echo "  -o FILE         Save response to file"
  echo "  -s              Use SharePoint REST API instead of Microsoft Graph"
  echo "  -H HEADER       Add custom header (can be used multiple times)"
  echo "  --site-url URL  Override site URL for SharePoint REST API (with -s flag)"
  echo ""
  echo "Microsoft Graph Examples (default):"
  echo "  # Get root site"
  echo "  $0 \"sites/root\""
  echo ""
  echo "  # Get site by path"
  echo "  $0 \"sites/{tenant}.sharepoint.com:/sites/MySite\""
  echo ""
  echo "  # List all lists in a site"
  echo "  $0 \"sites/{site-id}/lists\""
  echo ""
  echo "  # Get items from a list"
  echo "  $0 \"sites/{site-id}/lists/{list-id}/items?\\\$expand=fields\""
  echo ""
  echo "  # Get files in a document library"
  echo "  $0 \"sites/{site-id}/drive/root/children\""
  echo ""
  echo "  # Search for files"
  echo "  $0 \"sites/{site-id}/drive/root/search(q='report')\""
  echo ""
  echo "  # Create a list item"
  echo "  $0 \"sites/{site-id}/lists/{list-id}/items\" -m POST -d '{\"fields\": {\"Title\": \"New Item\"}}'"
  echo ""
  echo "SharePoint REST API Examples (-s flag):"
  echo "  # Get site info"
  echo "  $0 \"web\" -s"
  echo ""
  echo "  # Get all lists"
  echo "  $0 \"web/lists\" -s"
  echo ""
  echo "  # Get list items"
  echo "  $0 \"web/lists/getbytitle('Documents')/items\" -s"
  echo ""
  echo "  # Delete a subsite (DANGEROUS!)"
  echo "  $0 \"web\" -m DELETE -s --site-url \"https://tenant.sharepoint.com/sites/ParentSite/SubSite\""
  echo ""
  exit 1
fi

# Function to get OAuth access token for Microsoft Graph (ROPC flow)
get_graph_token() {
  local TOKEN_URL="https://login.windows.net/common/oauth2/token"
  local CACHE_FILE="./.token_cache_sharepoint_graph"

  # Check if we have a cached token
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

  # Get new token using ROPC (password) flow
  local RESPONSE=$(curl -s -X POST "$TOKEN_URL" \
    -d "grant_type=password" \
    -d "client_id=$SHAREPOINT_CLIENT_ID" \
    -d "client_secret=$SHAREPOINT_CLIENT_SECRET" \
    -d "username=$SHAREPOINT_USERNAME" \
    -d "password=$SHAREPOINT_PASSWORD" \
    -d "resource=https://graph.microsoft.com")

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

# Function to get OAuth access token for SharePoint REST API (ROPC flow)
get_sharepoint_token() {
  if [ -z "$SHAREPOINT_SITE_URL" ]; then
    echo -e "${RED}ERROR: SHAREPOINT_SITE_URL required for SharePoint REST API${NC}" >&2
    return 1
  fi

  # Extract the base SharePoint URL (e.g., https://tenant.sharepoint.com)
  local SP_BASE_URL=$(echo "$SHAREPOINT_SITE_URL" | grep -oP 'https://[^/]+')
  local TOKEN_URL="https://login.windows.net/common/oauth2/token"
  local CACHE_FILE="./.token_cache_sharepoint_rest"

  # Check if we have a cached token
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

  # Get new token using ROPC (password) flow
  local RESPONSE=$(curl -s -X POST "$TOKEN_URL" \
    -d "grant_type=password" \
    -d "client_id=$SHAREPOINT_CLIENT_ID" \
    -d "client_secret=$SHAREPOINT_CLIENT_SECRET" \
    -d "username=$SHAREPOINT_USERNAME" \
    -d "password=$SHAREPOINT_PASSWORD" \
    -d "resource=$SP_BASE_URL")

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

# Get access token based on API type
echo -e "${CYAN}Authenticating...${NC}" >&2
if [ "$USE_GRAPH" = true ]; then
  TOKEN=$(get_graph_token)
else
  TOKEN=$(get_sharepoint_token)
fi

if [ $? -ne 0 ]; then
  exit 1
fi
echo -e "${GREEN}✓ Authenticated${NC}" >&2
echo "" >&2

# Build full URL
if [[ "$ENDPOINT" == http* ]]; then
  # Full URL provided (e.g., from @odata.nextLink) - use as-is
  FULL_URL="$ENDPOINT"
else
  # URL encode problematic characters
  ENDPOINT="${ENDPOINT//%/%25}"      # Percent (must be first!)
  ENDPOINT="${ENDPOINT// /%20}"      # Space
  ENDPOINT="${ENDPOINT//#/%23}"      # Hash
  ENDPOINT="${ENDPOINT//+/%2B}"      # Plus

  if [ "$USE_GRAPH" = true ]; then
    # Microsoft Graph API
    FULL_URL="https://graph.microsoft.com/v1.0/${ENDPOINT#/}"
  else
    # SharePoint REST API
    SITE_URL="${OVERRIDE_SITE_URL:-$SHAREPOINT_SITE_URL}"
    # URL encode the site URL
    SITE_URL="${SITE_URL// /%20}"
    FULL_URL="${SITE_URL}/_api/${ENDPOINT#/}"
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

# Safety check for DELETE operations
if [ "$METHOD" = "DELETE" ] && [ "$USE_GRAPH" = false ]; then
  # For SharePoint REST API deletions, ensure we're deleting a deeply nested subsite
  # Pattern: /sites/MyVAF2025/YYYY/XX/... (at least 4 path segments after domain)
  SITE_URL_FOR_CHECK="${OVERRIDE_SITE_URL:-$SHAREPOINT_SITE_URL}"
  
  # Extract path from URL (remove protocol and domain)
  SITE_PATH=$(echo "$SITE_URL_FOR_CHECK" | sed 's|https\?://[^/]*/||')
  
  # Count path segments (e.g., "sites/MyVAF2025/2025/FI/SubSite" = 5 segments)
  SEGMENT_COUNT=$(echo "$SITE_PATH" | awk -F'/' '{print NF}')
  
  # Require at least 5 segments: sites/MyVAF2025/YYYY/XX/SubSite
  if [ "$SEGMENT_COUNT" -lt 5 ]; then
    echo -e "${RED}ERROR: DELETE operation blocked for safety${NC}" >&2
    echo "" >&2
    echo "Site path '$SITE_PATH' has only $SEGMENT_COUNT segments." >&2
    echo "Minimum required: 5 segments (e.g., sites/MyVAF2025/2025/FI/SubSite)" >&2
    echo "" >&2
    echo "This prevents accidental deletion of parent sites." >&2
    echo "" >&2
    exit 1
  fi
  
  echo -e "${YELLOW}⚠ DELETE operation detected${NC}" >&2
  echo -e "${YELLOW}Target site: $SITE_URL_FOR_CHECK${NC}" >&2
  echo -e "${YELLOW}Path segments: $SEGMENT_COUNT (minimum 5 required)${NC}" >&2
  echo "" >&2
fi

# Build curl command based on method
CURL_CMD=(curl -s -w "\n%{http_code}")

# Add headers
CURL_CMD+=(-H "Authorization: Bearer $TOKEN")
CURL_CMD+=(-H "Accept: application/json")

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
    if [ "$USE_GRAPH" = true ]; then
      # Microsoft Graph API uses standard DELETE
      CURL_CMD+=(-X DELETE)
    else
      # SharePoint REST API uses POST with X-HTTP-Method: DELETE
      CURL_CMD+=(-X POST)
      CURL_CMD+=(-H "X-HTTP-Method: DELETE")
      CURL_CMD+=(-H "Content-Length: 0")
    fi
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

# Print response
echo -e "${CYAN}Response (HTTP $HTTP_CODE):${NC}" >&2
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2

# If output file is specified, save to file
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

echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}" >&2

# Status and timing
if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
  echo -e "${GREEN}✓ Success${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC}" >&2
elif [ "$HTTP_CODE" -ge 400 ]; then
  echo -e "${RED}✗ Error${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC}" >&2
else
  echo -e "${YELLOW}⚠ Unexpected status${NC} (HTTP $HTTP_CODE) ${CYAN}⏱ ${DURATION}ms${NC}" >&2
fi

echo "" >&2

# Show record count for successful GET requests with 'value' array
if [ "$METHOD" = "GET" ] && [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
  RECORD_COUNT=$(echo "$RESPONSE_DATA" | jq '.value | length' 2>/dev/null)
  if [ -n "$RECORD_COUNT" ] && [ "$RECORD_COUNT" != "null" ]; then
    echo -e "${CYAN}Records returned: $RECORD_COUNT${NC}" >&2
    echo "" >&2
  fi
fi

# Exit with appropriate code
if [ "$HTTP_CODE" -ge 200 ] && [ "$HTTP_CODE" -lt 300 ]; then
  exit 0
else
  exit 1
fi
