# Lua Transform Guide

Complete guide to implementing custom data transformations for Dynamics 365 migrations using Lua scripts.

---

## Table of Contents

- [Overview](#overview)
- [Script Structure](#script-structure)
- [The Declaration Phase](#the-declaration-phase)
- [The Transform Phase](#the-transform-phase)
- [Operation Types](#operation-types)
- [Standard Library Reference](#standard-library-reference)
- [Complete Examples](#complete-examples)
- [Best Practices](#best-practices)
- [Debugging](#debugging)

---

## Overview

Lua transform mode provides a **scriptable alternative to declarative field mappings** for complex data migrations. Instead of defining static field-to-field mappings, you write Lua code that:

1. **Declares** what data to fetch from source and target environments
2. **Transforms** the data into create/update/delete/deactivate operations

This approach is ideal for:
- Complex business logic (conditional transformations)
- Data flattening or aggregation
- Lookups and cross-entity matching
- Scenarios requiring full programmatic control

### Script Execution Flow

```
┌──────────────────┐
│   Your Script    │
│   (module M)     │
└────────┬─────────┘
         │
    ┌────▼────────────────────────┐
    │  M.declare()                │
    │  → Returns data requirements│
    └────┬────────────────────────┘
         │
    ┌────▼────────────────────────┐
    │  Fetch source data          │
    │  Fetch target data          │
    └────┬────────────────────────┘
         │
    ┌────▼────────────────────────┐
    │  M.transform(source, target)│
    │  → Returns operations       │
    └────┬────────────────────────┘
         │
    ┌────▼────────────────────────┐
    │  Execute operations         │
    │  (create/update/delete)     │
    └─────────────────────────────┘
```

---

## Script Structure

All Lua transform scripts follow the **Neovim/Lua module convention**:

```lua
local M = {}

-- Phase 1: Declare what data we need
function M.declare()
    return {
        source = { ... },
        target = { ... }
    }
end

-- Phase 2: Transform data into operations
function M.transform(source, target)
    local ops = {}
    -- Your transformation logic here
    return ops
end

return M
```

### Required Components

| Component | Type | Required | Purpose |
|-----------|------|----------|---------|
| `M` | table | ✅ | Module container |
| `M.declare` | function | ✅ | Declares data requirements |
| `M.transform` | function | ✅ | Transforms data to operations |
| `return M` | statement | ✅ | Exports the module |

---

## The Declaration Phase

The `M.declare()` function tells the engine **what data to fetch** before transformation.

### Declaration Structure

```lua
function M.declare()
    return {
        source = {
            entity_name = {
                fields = { "field1", "field2", ... },
                expand = { "lookup_field", ... },
                filter = "OData filter expression",
                top = 1000  -- Maximum records to fetch
            },
            -- More entities...
        },
        target = {
            entity_name = {
                fields = { "field1", "field2", ... },
                filter = "OData filter expression"
            },
            -- More entities...
        }
    }
end
```

### Entity Declaration Options

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fields` | array | ✅ | List of field logical names to retrieve |
| `expand` | array | ❌ | Navigation properties to expand (includes related records) |
| `filter` | string | ❌ | OData filter expression to limit records |
| `top` | number | ❌ | Maximum number of records to fetch |

### Example: Simple Declaration

```lua
function M.declare()
    return {
        source = {
            account = {
                fields = { "accountid", "name", "revenue" },
                filter = "statecode eq 0"  -- Active only
            }
        },
        target = {
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end
```

### Example: Declaration with Lookups

```lua
function M.declare()
    return {
        source = {
            contact = {
                fields = { 
                    "contactid", 
                    "firstname", 
                    "lastname", 
                    "_parentcustomerid_value"  -- Lookup field
                },
                filter = "statecode eq 0",
                top = 5000
            },
            account = {
                fields = { "accountid", "name" },
                filter = "statecode eq 0"
            }
        },
        target = {
            contact = {
                fields = { "contactid", "firstname", "lastname" }
            },
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end
```

### OData Filter Syntax

Filters use OData v4 query syntax:

| Operator | Example | Description |
|----------|---------|-------------|
| `eq` | `statecode eq 0` | Equals |
| `ne` | `name ne null` | Not equals |
| `gt` | `revenue gt 1000000` | Greater than |
| `ge` | `revenue ge 1000000` | Greater or equal |
| `lt` | `createdon lt 2024-01-01` | Less than |
| `le` | `createdon le 2024-01-01` | Less or equal |
| `and` | `statecode eq 0 and revenue gt 0` | Logical AND |
| `or` | `statecode eq 0 or statecode eq 1` | Logical OR |
| `not` | `not (statecode eq 2)` | Logical NOT |
| `contains` | `contains(name, 'Corp')` | String contains |
| `startswith` | `startswith(name, 'A')` | String starts with |
| `endswith` | `endswith(emailaddress1, '@example.com')` | String ends with |

---

## The Transform Phase

The `M.transform(source, target)` function receives the fetched data and returns a list of **operations** to execute.

### Function Signature

```lua
function M.transform(source, target)
    -- source: table of entity_name -> array of records
    -- target: table of entity_name -> array of records
    
    local ops = {}
    
    -- Your logic here
    
    return ops  -- Array of operation tables
end
```

### Data Structure

Both `source` and `target` are tables structured as:

```lua
{
    entity_name = {
        {
            fieldname1 = value1,
            fieldname2 = value2,
            ...
        },
        -- More records...
    },
    -- More entities...
}
```

### Accessing Records

```lua
function M.transform(source, target)
    -- Get all source accounts
    local source_accounts = source.account or {}
    
    -- Iterate over contacts
    for _, contact in ipairs(source.contact or {}) do
        local name = contact.firstname or ""
        local email = contact.emailaddress1 or ""
        
        -- Your logic here
    end
    
    return {}
end
```

---

## Operation Types

Operations tell the engine what actions to perform on records.

### 1. Create

Creates a new record in the target environment.

```lua
{
    entity = "account",
    operation = "create",
    fields = {
        name = "Acme Corp",
        revenue = 1000000,
        industrycode = 1  -- OptionSet value
    }
}
```

**Required fields:**
- `entity` (string) - Target entity logical name
- `operation` (string) - Must be `"create"`
- `fields` (table) - Field name/value pairs

**Optional fields:**
- `id` (string) - GUID to use for the new record (usually omitted - auto-generated)

### 2. Update

Updates an existing record in the target environment.

```lua
{
    entity = "account",
    operation = "update",
    id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    fields = {
        revenue = 2000000,
        description = "Updated description"
    }
}
```

**Required fields:**
- `entity` (string) - Target entity logical name
- `operation` (string) - Must be `"update"`
- `id` (string) - GUID of the record to update
- `fields` (table) - Field name/value pairs to update

### 3. Delete

Permanently deletes a record from the target environment.

```lua
{
    entity = "contact",
    operation = "delete",
    id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Required fields:**
- `entity` (string) - Target entity logical name
- `operation` (string) - Must be `"delete"`
- `id` (string) - GUID of the record to delete

### 4. Deactivate

Sets `statecode = 1` (deactivated) on a record.

```lua
{
    entity = "account",
    operation = "deactivate",
    id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Required fields:**
- `entity` (string) - Target entity logical name
- `operation` (string) - Must be `"deactivate"`
- `id` (string) - GUID of the record to deactivate

### 5. Skip

Marks a record to be skipped (for audit/logging purposes).

```lua
{
    entity = "contact",
    operation = "skip",
    id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890",  -- Optional
    reason = "Missing required email address"
}
```

**Required fields:**
- `entity` (string) - Entity logical name
- `operation` (string) - Must be `"skip"`

**Optional fields:**
- `id` (string) - Record GUID
- `reason` (string) - Why the record was skipped

### 6. Error

Marks a record as having an error (for audit/logging purposes).

```lua
{
    entity = "account",
    operation = "error",
    id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890",  -- Optional
    error = "Invalid revenue value: -1000"
}
```

**Required fields:**
- `entity` (string) - Entity logical name
- `operation` (string) - Must be `"error"`

**Optional fields:**
- `id` (string) - Record GUID
- `error` (string) - Error message

### Lookup Field Syntax

Lookup fields (EntityReference in Dynamics) use the field's logical name without the `_value` suffix:

```lua
{
    entity = "contact",
    operation = "create",
    fields = {
        firstname = "John",
        lastname = "Doe",
        parentcustomerid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"  -- Account GUID
    }
}
```

The engine automatically converts this to the OData `@odata.bind` format during execution.

---

## Standard Library Reference

The `lib.*` namespace provides helper functions for common operations.

### Collection Functions

#### `lib.find(records, field, value) -> record|nil`

Find the first record where `record[field] == value`.

```lua
local accounts = target.account or {}
local match = lib.find(accounts, "name", "Contoso")
if match then
    lib.log("Found account: " .. match.accountid)
end
```

#### `lib.filter(records, predicate_fn) -> records`

Filter records by a predicate function.

```lua
local accounts = source.account or {}
local big_accounts = lib.filter(accounts, function(acc)
    return (acc.revenue or 0) > 1000000
end)
```

#### `lib.map(records, transform_fn) -> records`

Transform each record using a function.

```lua
local contacts = source.contact or {}
local names = lib.map(contacts, function(c)
    return c.firstname .. " " .. c.lastname
end)
```

#### `lib.group_by(records, field) -> table`

Group records by a field's value.

```lua
local contacts = source.contact or {}
local by_company = lib.group_by(contacts, "_parentcustomerid_value")

-- Access groups
for company_id, company_contacts in pairs(by_company) do
    lib.log("Company " .. company_id .. " has " .. #company_contacts .. " contacts")
end
```

### GUID Functions

#### `lib.guid() -> string`

Generate a new random GUID.

```lua
local new_id = lib.guid()
-- e.g., "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
```

#### `lib.is_guid(value) -> bool`

Check if a value is a valid GUID string.

```lua
if lib.is_guid(account.accountid) then
    -- Process the ID
end
```

### String Functions

#### `lib.lower(s) -> string`

Convert string to lowercase.

```lua
local email = lib.lower(contact.emailaddress1 or "")
```

#### `lib.upper(s) -> string`

Convert string to uppercase.

```lua
local code = lib.upper(account.accountnumber or "")
```

#### `lib.trim(s) -> string`

Remove leading and trailing whitespace.

```lua
local name = lib.trim(contact.firstname or "")
```

#### `lib.split(s, delimiter) -> array`

Split a string by delimiter.

```lua
local parts = lib.split("john.doe@example.com", "@")
-- parts[1] = "john.doe"
-- parts[2] = "example.com"
```

#### `lib.contains(s, substring) -> bool`

Check if string contains substring.

```lua
if lib.contains(account.name or "", "Corp") then
    -- Handle corporate accounts
end
```

#### `lib.starts_with(s, prefix) -> bool`

Check if string starts with prefix.

```lua
if lib.starts_with(contact.firstname or "", "A") then
    -- First name starts with 'A'
end
```

#### `lib.ends_with(s, suffix) -> bool`

Check if string ends with suffix.

```lua
if lib.ends_with(email or "", "@example.com") then
    -- Internal employee
end
```

### Date Functions

#### `lib.now() -> string`

Get current UTC time in ISO 8601 format.

```lua
local timestamp = lib.now()
-- e.g., "2024-12-22T14:30:00Z"
```

#### `lib.parse_date(s) -> string|nil`

Parse various date formats to ISO 8601.

```lua
local iso = lib.parse_date("12/25/2024")
-- Returns: "2024-12-25T00:00:00Z"

local iso2 = lib.parse_date("2024-12-25 15:30:00")
-- Returns: "2024-12-25T15:30:00Z"
```

Supported formats:
- `%Y-%m-%dT%H:%M:%SZ`
- `%Y-%m-%d`
- `%d/%m/%Y`
- `%m/%d/%Y`
- And more...

#### `lib.format_date(datetime, format) -> string|nil`

Format an ISO datetime string.

```lua
local formatted = lib.format_date("2024-12-25T15:30:00Z", "%Y-%m-%d")
-- Returns: "2024-12-25"
```

### Type Check Functions

#### `lib.is_nil(v) -> bool`

Check if value is nil.

```lua
if lib.is_nil(contact.emailaddress1) then
    lib.warn("Contact has no email")
end
```

#### `lib.is_string(v) -> bool`

Check if value is a string.

#### `lib.is_number(v) -> bool`

Check if value is a number.

#### `lib.is_table(v) -> bool`

Check if value is a table.

#### `lib.is_boolean(v) -> bool`

Check if value is a boolean.

### Logging Functions

#### `lib.log(message)`

Log an info message (captured and displayed after execution).

```lua
lib.log("Processing " .. #contacts .. " contacts")
```

#### `lib.warn(message)`

Log a warning message.

```lua
lib.warn("Contact " .. contact_id .. " has no email address")
```

### Progress Functions

#### `lib.status(message)`

Update the status display (shown in real-time during execution).

```lua
lib.status("Processing accounts...")
```

#### `lib.progress(current, total)`

Update the progress bar.

```lua
for i, contact in ipairs(contacts) do
    -- Process contact...
    
    if i % 10 == 0 then
        lib.progress(i, #contacts)
    end
end
```

Combined example:
```lua
lib.status("Deleting inactive contacts")
lib.progress(50, 100)
-- UI shows: "Deleting inactive contacts - 50/100 (50.0%)"
```

---

## Complete Examples

### Example 1: Simple Update Migration

Updates account revenue values from source to target.

```lua
local M = {}

function M.declare()
    return {
        source = {
            account = {
                fields = { "accountid", "name", "revenue" },
                filter = "statecode eq 0"
            }
        },
        target = {
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end

function M.transform(source, target)
    lib.status("Starting account revenue update...")
    
    local source_accounts = source.account or {}
    local target_accounts = target.account or {}
    
    local ops = {}
    
    for i, src_account in ipairs(source_accounts) do
        -- Find matching target account by name
        local tgt_account = lib.find(target_accounts, "name", src_account.name)
        
        if tgt_account then
            table.insert(ops, {
                entity = "account",
                operation = "update",
                id = tgt_account.accountid,
                fields = {
                    revenue = src_account.revenue
                }
            })
        else
            table.insert(ops, {
                entity = "account",
                operation = "create",
                fields = {
                    name = src_account.name,
                    revenue = src_account.revenue
                }
            })
        end
        
        if i % 50 == 0 then
            lib.progress(i, #source_accounts)
        end
    end
    
    lib.log("Processed " .. #source_accounts .. " accounts")
    lib.log("Generated " .. #ops .. " operations")
    
    return ops
end

return M
```

### Example 2: Conditional Delete and Transform

Demonstrates delete operations and conditional logic.

```lua
local M = {}

function M.declare()
    return {
        source = {
            contact = {
                fields = { "contactid", "firstname", "lastname", "emailaddress1" },
                filter = "firstname ne null"
            }
        },
        target = {
            contact = {
                fields = { "contactid", "firstname", "lastname" }
            }
        }
    }
end

function M.transform(source, target)
    lib.status("Processing contacts...")
    
    local source_contacts = source.contact or {}
    local ops = {}
    
    local deleted = 0
    local updated = 0
    local created = 0
    
    for i, contact in ipairs(source_contacts) do
        local firstname = contact.firstname or ""
        
        -- Delete contacts with names starting with 'X'
        if lib.starts_with(firstname, "X") then
            table.insert(ops, {
                entity = "contact",
                operation = "delete",
                id = contact.contactid
            })
            deleted = deleted + 1
            
        -- Update: Change 'Bob' to 'Robert'
        elseif firstname == "Bob" then
            table.insert(ops, {
                entity = "contact",
                operation = "update",
                id = contact.contactid,
                fields = {
                    firstname = "Robert"
                }
            })
            updated = updated + 1
            
        -- Skip contacts without email
        elseif lib.is_nil(contact.emailaddress1) then
            table.insert(ops, {
                entity = "contact",
                operation = "skip",
                id = contact.contactid,
                reason = "No email address"
            })
            
        -- Create new contact
        else
            table.insert(ops, {
                entity = "contact",
                operation = "create",
                fields = {
                    firstname = contact.firstname,
                    lastname = contact.lastname,
                    emailaddress1 = contact.emailaddress1
                }
            })
            created = created + 1
        end
        
        if i % 100 == 0 then
            lib.progress(i, #source_contacts)
        end
    end
    
    lib.log("Summary: Created=" .. created .. ", Updated=" .. updated .. ", Deleted=" .. deleted)
    
    return ops
end

return M
```

### Example 3: Lookup Resolution

Links contacts to accounts by matching on name fields.

```lua
local M = {}

function M.declare()
    return {
        source = {
            contact = {
                fields = { 
                    "contactid", 
                    "firstname", 
                    "lastname",
                    "_parentcustomerid_value"  -- Source account reference
                },
                filter = "statecode eq 0"
            },
            account = {
                fields = { "accountid", "name" },
                filter = "statecode eq 0"
            }
        },
        target = {
            contact = {
                fields = { "contactid", "firstname", "lastname" }
            },
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end

function M.transform(source, target)
    lib.status("Building lookup maps...")
    
    local source_contacts = source.contact or {}
    local source_accounts = source.account or {}
    local target_accounts = target.account or {}
    
    -- Build mapping: source account ID -> source account name
    local source_account_names = {}
    for _, acc in ipairs(source_accounts) do
        source_account_names[acc.accountid] = acc.name
    end
    
    -- Build mapping: account name -> target account ID
    local target_account_ids = {}
    for _, acc in ipairs(target_accounts) do
        local name_lower = lib.lower(acc.name or "")
        target_account_ids[name_lower] = acc.accountid
    end
    
    lib.status("Processing contacts with lookups...")
    
    local ops = {}
    local linked = 0
    local unlinked = 0
    
    for i, contact in ipairs(source_contacts) do
        local fields = {
            firstname = contact.firstname,
            lastname = contact.lastname
        }
        
        -- Resolve parent account lookup
        local source_parent_id = contact["_parentcustomerid_value"]
        if source_parent_id then
            local account_name = source_account_names[source_parent_id]
            if account_name then
                local name_lower = lib.lower(account_name)
                local target_account_id = target_account_ids[name_lower]
                
                if target_account_id then
                    -- Link to target account
                    fields.parentcustomerid = target_account_id
                    linked = linked + 1
                else
                    lib.warn("No matching target account for: " .. account_name)
                    unlinked = unlinked + 1
                end
            end
        end
        
        table.insert(ops, {
            entity = "contact",
            operation = "create",
            fields = fields
        })
        
        if i % 100 == 0 then
            lib.progress(i, #source_contacts)
        end
    end
    
    lib.log("Linked " .. linked .. " contacts to accounts")
    if unlinked > 0 then
        lib.warn(unlinked .. " contacts could not be linked")
    end
    
    return ops
end

return M
```

### Example 4: Data Aggregation

Aggregates child records into parent summary fields.

```lua
local M = {}

function M.declare()
    return {
        source = {
            opportunity = {
                fields = { 
                    "opportunityid", 
                    "_parentaccountid_value",
                    "estimatedvalue",
                    "statecode"
                },
                filter = "statecode eq 0"  -- Open opportunities
            }
        },
        target = {
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end

function M.transform(source, target)
    lib.status("Aggregating opportunity data...")
    
    local opportunities = source.opportunity or {}
    local target_accounts = target.account or {}
    
    -- Group opportunities by parent account
    local by_account = lib.group_by(opportunities, "_parentaccountid_value")
    
    local ops = {}
    
    for account_id, opps in pairs(by_account) do
        -- Calculate total value and count
        local total_value = 0
        local count = 0
        
        for _, opp in ipairs(opps) do
            total_value = total_value + (opp.estimatedvalue or 0)
            count = count + 1
        end
        
        -- Find target account
        local target_account = lib.find(target_accounts, "accountid", account_id)
        
        if target_account then
            table.insert(ops, {
                entity = "account",
                operation = "update",
                id = account_id,
                fields = {
                    -- Custom fields to store aggregated data
                    new_totalopenopportunities = count,
                    new_totalopenvalue = total_value
                }
            })
        else
            lib.warn("Account not found in target: " .. account_id)
        end
    end
    
    lib.log("Updated " .. #ops .. " accounts with opportunity summaries")
    
    return ops
end

return M
```

---

## Best Practices

### 1. Use Filters to Minimize Data

Fetch only what you need:

```lua
-- Good: Fetch only active accounts
account = {
    fields = { "accountid", "name" },
    filter = "statecode eq 0"
}

-- Bad: Fetch all accounts then filter in Lua
account = {
    fields = { "accountid", "name", "statecode" }
}
```

### 2. Check for nil Values

Always handle missing data:

```lua
-- Good
local email = contact.emailaddress1 or ""
if lib.is_nil(contact.parentcustomerid) then
    -- Handle missing lookup
end

-- Bad (will crash if field is nil)
local email = contact.emailaddress1
if email:len() > 0 then
    -- This will error if email is nil
end
```

### 3. Provide Progress Updates

For long-running transforms:

```lua
for i, record in ipairs(large_dataset) do
    -- Process record...
    
    if i % 100 == 0 then
        lib.status("Processing record " .. i .. "/" .. #large_dataset)
        lib.progress(i, #large_dataset)
    end
end
```

### 4. Log Important Decisions

Help with debugging and auditing:

```lua
if condition then
    lib.log("Skipping record " .. id .. ": condition met")
    table.insert(ops, {
        entity = "contact",
        operation = "skip",
        id = id,
        reason = "Condition met"
    })
end
```

### 5. Use Lookup Maps for Performance

Build indices instead of searching repeatedly:

```lua
-- Good: Build once, lookup many times
local account_by_name = {}
for _, acc in ipairs(target.account or {}) do
    account_by_name[lib.lower(acc.name or "")] = acc.accountid
end

for _, contact in ipairs(source.contact or {}) do
    local account_id = account_by_name[lib.lower(contact.company or "")]
    -- Fast lookup
end

-- Bad: Search for every contact
for _, contact in ipairs(source.contact or {}) do
    local account = lib.find(target.account, "name", contact.company)
    -- Slow: O(n) search repeated
end
```

### 6. Validate Operations

Use skip/error operations for validation:

```lua
if not lib.is_guid(account.accountid) then
    table.insert(ops, {
        entity = "account",
        operation = "error",
        id = account.accountid,
        error = "Invalid GUID format"
    })
else
    -- Process normally
end
```

### 7. Handle Edge Cases

```lua
-- Empty source data
local contacts = source.contact or {}
if #contacts == 0 then
    lib.warn("No contacts found in source")
    return {}
end

-- Duplicate handling
local seen_ids = {}
for _, contact in ipairs(contacts) do
    if seen_ids[contact.contactid] then
        lib.warn("Duplicate contact ID: " .. contact.contactid)
    else
        seen_ids[contact.contactid] = true
        -- Process contact
    end
end
```

---

## Debugging

### Validation Errors

The script validator will catch:
- Lua syntax errors (with line numbers)
- Missing `declare` or `transform` functions
- Invalid OData filters
- Missing required operation fields

### Runtime Logs

Check the log file for execution details:

```bash
tail -f dynamics-cli.log
```

### Using lib.log for Debugging

```lua
function M.transform(source, target)
    lib.log("Source accounts: " .. #(source.account or {}))
    lib.log("Target accounts: " .. #(target.account or {}))
    
    -- Debug a specific record
    local first = source.account[1]
    if first then
        for key, value in pairs(first) do
            lib.log("  " .. key .. " = " .. tostring(value))
        end
    end
    
    return {}
end
```

### Testing with Small Datasets

Use `top` to limit records during development:

```lua
function M.declare()
    return {
        source = {
            account = {
                fields = { "accountid", "name" },
                top = 10  -- Only fetch 10 for testing
            }
        },
        target = {}
    }
end
```

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Script must return a module table` | Missing `return M` | Add `return M` at end of script |
| `Script must have a 'declare' function` | Missing or misspelled `M.declare` | Check function name spelling |
| `Operation requires id` | Update/delete missing `id` field | Add `id = record_guid` |
| `Create operation requires fields` | Create with empty `fields` table | Add at least one field |
| `attempt to index a nil value` | Accessing field on nil record | Add nil checks: `record.field or ""` |

### Sandbox Restrictions

Your scripts run in a sandboxed environment. The following are **NOT available**:
- `io` (file I/O)
- `os` (system commands)
- `debug` (debugging functions)
- `package` / `require` (loading modules)
- `loadfile` / `dofile` (loading external code)

Use only:
- `table` (table manipulation)
- `string` (string functions)
- `math` (mathematical functions)
- `lib.*` (custom standard library)

---

## Summary

Lua transform mode provides complete flexibility for complex migrations:

1. **Declare** your data requirements in `M.declare()`
2. **Transform** the data into operations in `M.transform(source, target)`
3. Use the **standard library** (`lib.*`) for common operations
4. Return an array of **operations** (create/update/delete/deactivate/skip/error)
5. **Test** with small datasets and use `lib.log()` for debugging

For simple field-to-field mappings, use **Declarative mode** instead. Use Lua mode when you need:
- Conditional logic
- Lookups and matching
- Data aggregation
- Complex transformations

Happy scripting! 🚀
