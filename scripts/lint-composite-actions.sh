#!/usr/bin/env bash

set -euo pipefail

project_root() {
    local script_dir

    script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
    cd -- "$script_dir/.." && pwd
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: required command not found: $1" >&2
        exit 1
    fi
}

repo_root="$(project_root)"
actions_dir="$repo_root/.github/actions"

require_command ruby
require_command shellcheck

if [[ ! -d "$actions_dir" ]]; then
    exit 0
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

manifest_path="$tmp_dir/manifest.tsv"

ruby - "$tmp_dir" "$actions_dir" > "$manifest_path" <<'RUBY'
require 'yaml'

output_dir = ARGV.fetch(0)
actions_dir = ARGV.fetch(1)
counter = 0

Dir.glob(File.join(actions_dir, '**', 'action.y{a,}ml')).sort.each do |path|
  data = YAML.load_file(path)
  next unless data.is_a?(Hash)

  runs = data['runs']
  next unless runs.is_a?(Hash) && runs['using'] == 'composite'

  Array(runs['steps']).each_with_index do |step, index|
    next unless step.is_a?(Hash)

    script = step['run']
    next unless script.is_a?(String) && !script.empty?
    next unless step['shell'] == 'bash'

    sanitized_script = script.gsub(/\$\{\{.*?\}\}/m, '__GITHUB_ACTIONS_EXPRESSION__')

    counter += 1
    output_path = File.join(output_dir, format('%03d.sh', counter))
    File.write(output_path, sanitized_script)

    step_name = step['name'].to_s.gsub(/[\t\n]/, ' ')
    puts [output_path, path, index + 1, step_name].join("\t")
  end
end
RUBY

if [[ ! -s "$manifest_path" ]]; then
    echo "No composite action bash steps found."
    exit 0
fi

status=0
while IFS=$'\t' read -r extracted_script source_file step_index step_name; do
    echo "==> shellcheck: $source_file (step $step_index: ${step_name:-unnamed})"
    if ! shellcheck -s bash "$extracted_script"; then
        status=1
    fi
done < "$manifest_path"

exit "$status"