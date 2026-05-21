# Healthcare & Dental SaaS — Domain Standards

Reference document for SaaSStandards skill. Additional requirements for healthcare/dental SaaS applications.

---

## Healthcare SaaS Extends Generic SaaS

Everything in the generic SaaS standards applies, PLUS the following domain-specific requirements.

---

## Onboarding: Healthcare-Specific Fields

### Practice Information (Required — Step 2)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| practice_name | text | YES | Legal practice name |
| practice_type | select | YES | General, Orthodontics, Pediatric, Endodontics, Periodontics, Oral Surgery, Prosthodontics, Multi-Specialty |
| practice_phone | tel | YES | Primary contact number |
| practice_fax | tel | NO | Still used for insurance/referrals |
| practice_address | address | YES | Full address (street, city, state, zip) |
| practice_website | url | NO | For patient-facing features |
| tax_id | text | RECOMMENDED | Required for insurance billing |

### Provider Information (Required — Step 2 or 3)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| NPI_number | text | RECOMMENDED | 10-digit National Provider Identifier |
| license_number | text | RECOMMENDED | State dental license |
| license_state | select | RECOMMENDED | State of licensure |
| DEA_number | text | NO | Only if prescribing controlled substances |
| specialty_certifications | multi-select | NO | Board certifications |

### Team & Operations (Required — Step 2 or 3)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| number_of_providers | number | YES | Dentists + hygienists with clinical duties |
| number_of_staff | number | YES | Total staff (front desk, billing, assistants, etc.) |
| number_of_operatories | number | RECOMMENDED | Treatment rooms — affects scheduling |
| number_of_locations | number | YES | Multi-site support detection |
| office_hours | schedule | RECOMMENDED | Default schedule template |

### Insurance & Billing (Recommended — Step 3)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| accepts_insurance | boolean | YES | Determines billing workflows |
| primary_insurance_types | multi-select | RECOMMENDED | Delta Dental, MetLife, Cigna, Aetna, Guardian, BCBS, United, Humana |
| practice_management_software | select | RECOMMENDED | Dentrix, Eaglesoft, Open Dental, Curve, Denticon, Other |
| imaging_software | select | NO | Dexis, Schick, Carestream, Apteryx |
| clearinghouse | select | NO | Tesia, DentalXChange, NEA, Availity |

### Compliance (Required — Step 3 or Final)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| HIPAA_BAA_accepted | checkbox | YES | Business Associate Agreement — MANDATORY before any PHI access |
| HIPAA_officer_name | text | RECOMMENDED | Designated privacy/security officer |
| HIPAA_officer_email | email | RECOMMENDED | For compliance communications |
| data_backup_preference | select | RECOMMENDED | Cloud, local, hybrid |

---

## Staff Role Taxonomy (Dental)

When the app involves team management, use these standard roles:

| Role | Code | Permissions Level | Notes |
|------|------|-------------------|-------|
| Practice Owner | owner | Full access | Business owner, billing, all admin |
| Dentist | dentist | Clinical + charting | Treatment plans, clinical notes |
| Hygienist | hygienist | Clinical (limited) | Perio charting, cleanings, notes |
| Dental Assistant | assistant | Clinical (view) | Chair-side, limited charting |
| Office Manager | office_mgr | Admin + billing | Scheduling, billing, reports |
| Front Desk | front_desk | Scheduling + basic | Appointments, check-in/out |
| Billing Specialist | billing | Billing only | Claims, insurance, payments |
| Lab Technician | lab_tech | Lab orders only | Crown/bridge/denture tracking |

---

## HIPAA Considerations in Development

### Data at Rest
- All PHI (Protected Health Information) encrypted at rest
- Database encryption (AES-256)
- File storage encryption (S3 SSE or equivalent)

### Data in Transit
- TLS 1.2+ enforced everywhere
- No PHI in URL parameters
- No PHI in client-side logs or analytics

### Access Controls
- Role-based access (RBAC) enforced at API level
- Audit logging for all PHI access
- Session timeout (configurable, default 15 min for clinical)
- IP allowlisting (optional, for high-security practices)

### Data Handling Rules
- No PHI in error messages shown to users
- No PHI in browser console logs
- No PHI in email notifications (use opaque references)
- Patient data requests require authentication + authorization
- Data export must be encrypted
- Data deletion must be complete (right to erasure)

### BAA Requirement
- HIPAA Business Associate Agreement MUST be accepted before any PHI is stored
- This is a legal requirement, not a nice-to-have
- Acceptance tracked in database with timestamp and IP address
- BAA document accessible from settings at all times

---

## Multi-Location Support

If `number_of_locations > 1` during onboarding:

### Database Pattern

```sql
CREATE TABLE locations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID REFERENCES organizations(id),
  name VARCHAR(255) NOT NULL,
  address_line1 VARCHAR(255) NOT NULL,
  city VARCHAR(100) NOT NULL,
  state VARCHAR(50) NOT NULL,
  zip VARCHAR(20) NOT NULL,
  phone VARCHAR(20) NOT NULL,
  fax VARCHAR(20),
  is_primary BOOLEAN DEFAULT FALSE,
  operatories INTEGER DEFAULT 1,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### UX Pattern
- After main onboarding, prompt: "Add your locations"
- Primary location pre-filled from onboarding address
- Additional locations added via simple form
- Location switcher in app header/sidebar
- Reports can filter by location or show all

---

## Integration Readiness

### Practice Management Software Integration

The onboarding should detect PMS choice and offer:
- **Dentrix/Eaglesoft**: "We'll help you set up data import"
- **Open Dental**: "API integration available"
- **Curve/Denticon**: "Cloud sync available"
- **Other/None**: "Manual setup — we'll guide you"

This information collected during onboarding enables:
- Automatic data import wizard post-onboarding
- Integration-specific setup guides
- Compatibility warnings before commitment
