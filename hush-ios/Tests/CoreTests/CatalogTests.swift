import Foundation
import Testing
@testable import Hush

@Suite struct CatalogTests {
    let catalog = Fixture.catalog

    @Test func duplicateKeysStaySeparateRows() {
        let rows = catalog.matching(query: "database", project: nil)
        #expect(Set(rows.map(\.id)) == [Fixture.infraDatabase.id, Fixture.cafeDatabase.id])
    }

    @Test func queryMatchesProjectNamesIgnoringCaseAndDiacritics() {
        let rows = catalog.matching(query: "  CAFE ", project: nil)
        #expect(Set(rows.map(\.id)) == [Fixture.cafeDatabase.id, Fixture.stripe.id])
    }

    @Test func projectFilterKeepsOnlyThatProject() {
        #expect(catalog.matching(query: "", project: Fixture.infra.id) == [Fixture.infraDatabase])
        #expect(catalog.matching(query: "stripe", project: Fixture.infra.id).isEmpty)
    }

    @Test func emptyQueryKeepsEveryRowSortedByKey() {
        #expect(catalog.matching(query: "", project: nil).map(\.key) == ["DATABASE_URL", "DATABASE_URL", "STRIPE_KEY"])
    }

    @Test func detailDescriptionNeverShowsValue() {
        let printed = "\(Fixture.stripeDetail) \(String(reflecting: Fixture.stripeDetail))"
        var dumped = ""
        dump(Fixture.stripeDetail, to: &dumped)
        #expect(!printed.contains("sk_test_value"))
        #expect(!dumped.contains("sk_test_value"))
    }
}

@Suite struct NewSecretTests {
    @Test func trimsKeyAndKeepsValueAndNote() throws {
        let draft = try NewSecret(key: "  API_KEY \n", value: " v ", note: nil, projectID: Fixture.infra.id, in: Fixture.catalog)
        #expect(draft.key == "API_KEY")
        #expect(draft.value == " v ")
        #expect(draft.note == nil)
    }

    @Test(arguments: [
        ("   ", "v", Fixture.infra.id as ProjectID?, NewSecret.Invalid.emptyKey),
        ("K", "", Fixture.infra.id, .emptyValue),
        ("K", "v", nil, .missingProject),
        ("K", "v", ProjectID(UUID()), .unknownProject),
    ])
    func rejectsInvalidDrafts(key: String, value: String, project: ProjectID?, expected: NewSecret.Invalid) {
        #expect(throws: expected) {
            try NewSecret(key: key, value: value, note: "n", projectID: project, in: Fixture.catalog)
        }
    }
}
