{{/*
Common labels for all resources.
*/}}
{{- define "lean-devnet.labels" -}}
app.kubernetes.io/part-of: lean-devnet
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version }}
{{- end }}

{{/*
Selector labels for a specific client.
*/}}
{{- define "lean-devnet.selectorLabels" -}}
app: {{ .name }}
app.kubernetes.io/part-of: lean-devnet
{{- end }}
