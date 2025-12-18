-- Demo Lua Transform Script
-- Demonstrates: fetch, delete, update, create operations with lookups and status logging

local M = {}

-- Declare function - returns what we need from source and target
function M.declare()
    return {
        source = {
            contact = {
                fields = { "contactid", "firstname", "lastname", "emailaddress1", "_parentcustomerid_value" },
                filter = "firstname ne null"
            },
            account = {
                fields = { "accountid", "name" },
                filter = "statecode eq 0",
                top = 100  -- Only need a few accounts for lookup matching
            }
        },
        target = {
            contact = {
                fields = { "contactid", "firstname", "lastname", "emailaddress1" }
            },
            account = {
                fields = { "accountid", "name" }
            }
        }
    }
end

-- Transform function - called by the runtime with fetched data
function M.transform(source, target)
    lib.status("Starting contact transformation...")
    
    local contacts = source.contact or {}
    local source_accounts = source.account or {}
    local target_accounts = target.account or {}
    
    lib.log("Processing " .. #contacts .. " contacts, " .. #source_accounts .. " source accounts")
    
    -- Build a lookup map: source account name -> target account id
    local target_account_by_name = {}
    for _, acc in ipairs(target_accounts) do
        if acc.name then
            target_account_by_name[acc.name:lower()] = acc.accountid
        end
    end
    
    -- Build source account id -> name map for resolving contact's parent
    local source_account_name_by_id = {}
    for _, acc in ipairs(source_accounts) do
        if acc.accountid then
            source_account_name_by_id[acc.accountid] = acc.name
        end
    end
    
    local ops = {}
    local deleted = 0
    local updated = 0
    local linked = 0
    
    for i, contact in ipairs(contacts) do
        local firstname = contact.firstname or ""
        
        -- Delete contacts starting with 'A'
        if firstname:sub(1, 1):upper() == "A" then
            lib.status("Deleting contact: " .. firstname)
            table.insert(ops, {
                entity = "contact",
                operation = "delete",
                id = contact.contactid
            })
            deleted = deleted + 1
            
        -- Update contacts starting with 'B' -> change to 'C'
        elseif firstname:sub(1, 1):upper() == "B" then
            local new_firstname = "C" .. firstname:sub(2)
            lib.status("Updating contact: " .. firstname .. " -> " .. new_firstname)
            table.insert(ops, {
                entity = "contact",
                operation = "update",
                id = contact.contactid,
                fields = {
                    firstname = new_firstname
                }
            })
            updated = updated + 1
            
        -- For contacts starting with 'D', try to link to parent account by name
        elseif firstname:sub(1, 1):upper() == "D" then
            local parent_id = contact["_parentcustomerid_value"]
            if parent_id then
                -- Look up the source account name
                local account_name = source_account_name_by_id[parent_id]
                if account_name then
                    -- Find matching target account
                    local target_account_id = target_account_by_name[account_name:lower()]
                    if target_account_id then
                        lib.status("Linking contact " .. firstname .. " to account: " .. account_name)
                        table.insert(ops, {
                            entity = "contact",
                            operation = "update",
                            id = contact.contactid,
                            fields = {
                                -- This is a lookup field - will be converted to @odata.bind
                                parentcustomerid = target_account_id
                            }
                        })
                        linked = linked + 1
                    end
                end
            end
        end
        
        -- Report progress every 10 contacts
        if i % 10 == 0 then
            lib.progress(i, #contacts)
        end
    end
    
    -- Create a new test contact linked to first target account (if any)
    lib.status("Creating new test contact...")
    local new_contact = {
        entity = "contact",
        operation = "create",
        fields = {
            firstname = "LuaTest",
            lastname = "Generated",
            emailaddress1 = "luatest@example.com"
        }
    }
    
    -- Link to first available target account
    if #target_accounts > 0 then
        new_contact.fields.parentcustomerid = target_accounts[1].accountid
        lib.log("Linking new contact to account: " .. (target_accounts[1].name or "unknown"))
    end
    
    table.insert(ops, new_contact)
    
    -- Final summary
    lib.status("Transformation complete!")
    lib.log("Summary: Deleted " .. deleted .. ", Updated " .. updated .. ", Linked " .. linked .. ", Created 1")
    
    return ops
end

return M
