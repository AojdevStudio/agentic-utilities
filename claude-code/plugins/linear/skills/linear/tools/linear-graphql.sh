#!/usr/bin/env bash
# linear-graphql.sh - GraphQL helper for Linear fields not supported by linearis CLI
# Covers: dueDate, estimate (story points), projects, milestones, initiatives
# Requires: LINEAR_API_TOKEN environment variable or ~/.env.secrets

set -euo pipefail

# Load token if not already set
if [[ -z "${LINEAR_API_TOKEN:-}" ]]; then
  if [[ -f ~/.env.secrets ]]; then
    source ~/.env.secrets
  else
    echo '{"error": "LINEAR_API_TOKEN not set and ~/.env.secrets not found"}' >&2
    exit 1
  fi
fi

API_URL="https://api.linear.app/graphql"

graphql_request() {
  local query="$1"
  curl -s -X POST "$API_URL" \
    -H "Content-Type: application/json" \
    -H "Authorization: $LINEAR_API_TOKEN" \
    -d "$query"
}

cmd_set_due_date() {
  local issue_id="$1"
  local due_date="$2"

  graphql_request "$(cat <<EOF
{
  "query": "mutation { issueUpdate(id: \"${issue_id}\", input: { dueDate: \"${due_date}\" }) { success issue { id identifier title dueDate } } }"
}
EOF
)"
}

cmd_set_estimate() {
  local issue_id="$1"
  local estimate="$2"

  graphql_request "$(cat <<EOF
{
  "query": "mutation { issueUpdate(id: \"${issue_id}\", input: { estimate: ${estimate} }) { success issue { id identifier title estimate } } }"
}
EOF
)"
}

cmd_set_due_date_and_estimate() {
  local issue_id="$1"
  local due_date="$2"
  local estimate="$3"

  graphql_request "$(cat <<EOF
{
  "query": "mutation { issueUpdate(id: \"${issue_id}\", input: { dueDate: \"${due_date}\", estimate: ${estimate} }) { success issue { id identifier title dueDate estimate } } }"
}
EOF
)"
}

# --- Project commands ---

cmd_create_project() {
  local name="$1"
  local description="$2"
  local content="$3"
  local team_id="$4"
  local lead_id="$5"
  local priority="$6"
  local start_date="$7"
  local target_date="$8"

  local payload
  payload=$(python3 - "$name" "$description" "$content" "$team_id" "$lead_id" "$priority" "$start_date" "$target_date" <<'PYEOF'
import json, sys

name, description, content, team_id, lead_id = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
priority, start_date, target_date = int(sys.argv[6]), sys.argv[7], sys.argv[8]

mutation = (
    'mutation { projectCreate(input: { '
    f'name: {json.dumps(name)}, '
    f'description: {json.dumps(description)}, '
    f'content: {json.dumps(content)}, '
    f'teamIds: ["{team_id}"], '
    f'leadId: "{lead_id}", '
    f'priority: {priority}, '
    f'startDate: "{start_date}", '
    f'targetDate: "{target_date}", '
    'state: "started" '
    '}) { success project { id name priority startDate targetDate state } } }'
)

sys.stdout.write(json.dumps({"query": mutation}))
PYEOF
  )

  graphql_request "$payload"
}

cmd_update_project() {
  local project_id="$1"
  local field="$2"
  local value="$3"

  local payload
  payload=$(python3 - "$project_id" "$field" "$value" <<'PYEOF'
import json, sys

project_id, field, value = sys.argv[1], sys.argv[2], sys.argv[3]

numeric_fields = {"priority"}
# state uses string enum but Linear accepts it as a string
string_fields = {"name", "description", "content", "startDate", "targetDate", "state"}

if field in numeric_fields:
    input_val = f'{field}: {int(value)}'
elif field in string_fields:
    input_val = f'{field}: {json.dumps(value)}'
else:
    sys.stderr.write(json.dumps({"error": f"Unknown field: {field}"}))
    sys.exit(1)

mutation = (
    f'mutation {{ projectUpdate(id: "{project_id}", input: {{ {input_val} }}) '
    f'{{ success project {{ id name description priority startDate targetDate state }} }} }}'
)

sys.stdout.write(json.dumps({"query": mutation}))
PYEOF
  )

  graphql_request "$payload"
}

cmd_create_milestone() {
  local project_id="$1"
  local name="$2"
  local target_date="$3"
  local description="${4:-}"

  local payload
  payload=$(python3 - "$project_id" "$name" "$target_date" "$description" <<'PYEOF'
import json, sys

project_id, name, target_date, description = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]

input_parts = (
    f'projectId: "{project_id}", '
    f'name: {json.dumps(name)}, '
    f'targetDate: "{target_date}"'
)

if description:
    input_parts += f', description: {json.dumps(description)}'

mutation = (
    f'mutation {{ projectMilestoneCreate(input: {{ {input_parts} }}) '
    f'{{ success projectMilestone {{ id name targetDate }} }} }}'
)

sys.stdout.write(json.dumps({"query": mutation}))
PYEOF
  )

  graphql_request "$payload"
}

cmd_list_initiatives() {
  local payload
  payload=$(python3 -c "
import json, sys
query = '{ initiatives { nodes { id name description status targetDate } } }'
sys.stdout.write(json.dumps({'query': query}))
")

  graphql_request "$payload"
}

cmd_link_initiative() {
  local initiative_id="$1"
  local project_id="$2"

  local payload
  payload=$(python3 - "$initiative_id" "$project_id" <<'PYEOF'
import json, sys

initiative_id, project_id = sys.argv[1], sys.argv[2]

mutation = (
    f'mutation {{ initiativeToProjectCreate(input: {{ '
    f'initiativeId: "{initiative_id}", projectId: "{project_id}" '
    f'}}) {{ success initiativeToProject {{ id }} }} }}'
)

sys.stdout.write(json.dumps({"query": mutation}))
PYEOF
  )

  graphql_request "$payload"
}

cmd_create_initiative() {
  local name="$1"
  local description="$2"
  local owner_id="$3"
  local target_date="$4"
  local content="$5"

  local payload
  payload=$(python3 - "$name" "$description" "$owner_id" "$target_date" "$content" <<'PYEOF'
import json, sys

name, description, owner_id, target_date, content = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

mutation = (
    'mutation { initiativeCreate(input: { '
    f'name: {json.dumps(name)}, '
    f'description: {json.dumps(description)}, '
    f'ownerId: "{owner_id}", '
    f'targetDate: "{target_date}", '
    f'content: {json.dumps(content)}, '
    'status: "Active" '
    '}) { success initiative { id name status targetDate } } }'
)

sys.stdout.write(json.dumps({"query": mutation}))
PYEOF
  )

  graphql_request "$payload"
}

# --- Main ---
case "${1:-}" in
  set-due-date)
    [[ $# -lt 3 ]] && { echo "Usage: $0 set-due-date <issueUUID> <YYYY-MM-DD>"; exit 1; }
    cmd_set_due_date "$2" "$3"
    ;;
  set-estimate)
    [[ $# -lt 3 ]] && { echo "Usage: $0 set-estimate <issueUUID> <points>"; exit 1; }
    cmd_set_estimate "$2" "$3"
    ;;
  set-due-date-and-estimate)
    [[ $# -lt 4 ]] && { echo "Usage: $0 set-due-date-and-estimate <issueUUID> <YYYY-MM-DD> <points>"; exit 1; }
    cmd_set_due_date_and_estimate "$2" "$3" "$4"
    ;;
  create-project)
    [[ $# -lt 9 ]] && { echo "Usage: $0 create-project '<name>' '<description>' '<content>' '<teamId>' '<leadId>' <priority> '<startDate>' '<targetDate>'"; echo "  Priority: 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low"; echo "  Dates: YYYY-MM-DD format"; exit 1; }
    cmd_create_project "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9"
    ;;
  update-project)
    [[ $# -lt 4 ]] && { echo "Usage: $0 update-project '<projectId>' '<field>' '<value>'"; echo "  Fields: name, description, content, priority, startDate, targetDate, state"; echo "  State: planned, started, paused, completed, canceled"; exit 1; }
    cmd_update_project "$2" "$3" "$4"
    ;;
  create-milestone)
    [[ $# -lt 4 ]] && { echo "Usage: $0 create-milestone '<projectId>' '<name>' '<targetDate>' ['<description>']"; exit 1; }
    cmd_create_milestone "$2" "$3" "$4" "${5:-}"
    ;;
  list-initiatives)
    cmd_list_initiatives
    ;;
  link-initiative)
    [[ $# -lt 3 ]] && { echo "Usage: $0 link-initiative '<initiativeId>' '<projectId>'"; exit 1; }
    cmd_link_initiative "$2" "$3"
    ;;
  create-initiative)
    [[ $# -lt 6 ]] && { echo "Usage: $0 create-initiative '<name>' '<description>' '<ownerId>' '<targetDate>' '<content>'"; exit 1; }
    cmd_create_initiative "$2" "$3" "$4" "$5" "$6"
    ;;
  *)
    echo "Usage: $0 <command> <args...>"
    echo ""
    echo "Issue commands:"
    echo "  set-due-date <issueUUID> <YYYY-MM-DD>"
    echo "  set-estimate <issueUUID> <points>"
    echo "  set-due-date-and-estimate <issueUUID> <YYYY-MM-DD> <points>"
    echo ""
    echo "Project commands:"
    echo "  create-project '<name>' '<desc>' '<content>' '<teamId>' '<leadId>' <priority> '<startDate>' '<targetDate>'"
    echo "  update-project '<projectId>' '<field>' '<value>'"
    echo "  create-milestone '<projectId>' '<name>' '<targetDate>' ['<description>']"
    echo ""
    echo "Initiative commands:"
    echo "  list-initiatives"
    echo "  create-initiative '<name>' '<description>' '<ownerId>' '<targetDate>' '<content>'"
    echo "  link-initiative '<initiativeId>' '<projectId>'"
    exit 1
    ;;
esac
